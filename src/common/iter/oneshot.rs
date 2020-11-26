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
#![allow(dead_code)]

/// Provides an iterator that yields the item only once
struct OneShot<'a, T> {
    item: &'a T,
    yielded: bool,
}

impl<'a, T> OneShot<'a, T> {
    pub fn new(item: &'a T) -> Self {
        Self {
            item,
            yielded: false,
        }
    }
}

impl<'a, T> Iterator for OneShot<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.yielded {
            self.yielded = true;
            Some(&self.item)
        } else {
            None
        }
    }
}
