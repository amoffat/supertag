/*
 * Supertag
 * Copyright (C) 2020 Andrew Moffat
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use crate::common::err::ParseOctalError;
use core::fmt;
use libc::mode_t;
use rusqlite::types::ToSqlOutput;
use rusqlite::{Error, ToSql};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Debug, Display};
use std::str::FromStr;

/// `ClassPerms` represents an individual "class" of permissions, like the owner class, group class,
/// or other class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ClassPerms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UMask(pub mode_t);

impl UMask {
    pub fn file_perms(&self) -> Permissions {
        (0o666 & (!self.0)).into()
    }

    pub fn dir_perms(&self) -> Permissions {
        (0o777 & (!self.0)).into()
    }
}

impl Default for UMask {
    fn default() -> Self {
        unsafe {
            // FIXME race!
            // potentially one way to fix the race is with fork() and communicating the inherited
            // umask back out to the parent.  only do this if it this function isn't called a lot
            // though, as forking is expensive.  another alternative, on linux, is a /proc file
            // (I forget which one) that has the current umask
            let cur_umask = libc::umask(0);
            libc::umask(cur_umask);
            Self(cur_umask)
        }
    }
}

impl Debug for UMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        Display::fmt(self, f)
    }
}

impl Display for UMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "UMask({:03o})", self.0)
    }
}

impl From<mode_t> for UMask {
    fn from(umask: mode_t) -> Self {
        Self(umask)
    }
}

impl From<UMask> for mode_t {
    fn from(um: UMask) -> Self {
        um.0
    }
}

impl ClassPerms {
    #[allow(dead_code)]
    pub fn new(read: bool, write: bool, execute: bool) -> Self {
        Self {
            read,
            write,
            execute,
        }
    }

    #[allow(dead_code)]
    pub fn all_perms(&mut self) {
        self.read = true;
        self.write = true;
        self.execute = true;
    }

    #[allow(dead_code)]
    pub fn all_but_write(&mut self) {
        self.all_perms();
        self.write = false;
    }

    pub fn mode(&self) -> mode_t {
        let mut val = 0;
        if self.read {
            val |= 4;
        }
        if self.write {
            val |= 2;
        }
        if self.execute {
            val |= 1;
        }
        val
    }
}

impl From<mode_t> for ClassPerms {
    fn from(val: mode_t) -> Self {
        Self {
            read: (val & 0b100) > 0,
            write: (val & 0b010) > 0,
            execute: (val & 0b001) > 0,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Permissions {
    owner: ClassPerms,
    group: ClassPerms,
    others: ClassPerms,
}

impl Debug for Permissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        Display::fmt(self, f)
    }
}

impl Display for Permissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.octal_string())
    }
}

impl Default for Permissions {
    fn default() -> Self {
        UMask::default().file_perms()
    }
}

impl Permissions {
    pub fn mode(&self) -> mode_t {
        (self.owner.mode() << 6) | (self.group.mode() << 3) | self.others.mode()
    }
    pub fn octal_string(&self) -> String {
        format!("{:03o}", self.mode())
    }
}

/// Conversion from octal
impl From<mode_t> for Permissions {
    fn from(val: mode_t) -> Self {
        Self {
            owner: ((val & libc::S_IRWXU) >> 6).into(),
            group: ((val & libc::S_IRWXG) >> 3).into(),
            others: (val & libc::S_IRWXO).into(),
        }
    }
}

impl ToSql for Permissions {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, Error> {
        Ok(self.mode().into())
    }
}

impl FromStr for Permissions {
    type Err = ParseOctalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(mode_t::from_str_radix(s, 8)
            .map_err(|_| ParseOctalError {})?
            .into())
    }
}

struct PermissionVisitor;

impl<'de> Visitor<'de> for PermissionVisitor {
    type Value = Permissions;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an octal umask value")
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(mode_t::from_str_radix(&v, 8)
            .map_err(|_| E::custom(format!("Invalid octal: {}", v)))?
            .into())
    }
}

impl Serialize for Permissions {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:o}", self.mode()))
    }
}

impl<'de> Deserialize<'de> for Permissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(PermissionVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_individual_perm() {
        // macro-ing this might be nice
        assert_eq!(ClassPerms::new(false, false, false).mode(), 0b000);
        assert_eq!(ClassPerms::new(false, false, true).mode(), 0b001);
        assert_eq!(ClassPerms::new(false, true, false).mode(), 0b010);
        assert_eq!(ClassPerms::new(true, false, false).mode(), 0b100);
        assert_eq!(ClassPerms::new(true, false, true).mode(), 0b101);
        assert_eq!(ClassPerms::new(true, true, false).mode(), 0b110);
        assert_eq!(ClassPerms::new(true, true, true).mode(), 0b111);
        assert_eq!(ClassPerms::new(false, true, true).mode(), 0b011);
    }

    #[test]
    fn test_individual_perm_from_int() {
        let perm: ClassPerms = 0b101.into();
        assert!(perm.read);
        assert!(!perm.write);
        assert!(perm.execute);
    }

    #[test]
    fn test_perm_to_int() {
        let perms: Permissions = 0o664.into();
        assert_eq!(perms.mode(), 0o664);
    }

    #[test]
    fn test_perm_from_int() {
        let perms: Permissions = 0o755.into();
        assert!(perms.owner.read);
        assert!(perms.owner.write);
        assert!(perms.owner.execute);

        assert!(perms.group.read);
        assert!(!perms.group.write);
        assert!(perms.group.execute);

        assert!(perms.others.read);
        assert!(!perms.others.write);
        assert!(perms.others.execute);
    }
}
