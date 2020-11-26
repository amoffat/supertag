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
use byteorder::{BigEndian, ByteOrder};
use std::path::Path;

/// Resource fork stuff

pub fn generate_icns_fork(icns: &Path) -> std::io::Result<Vec<u8>> {
    let icns_data = std::fs::read(icns)?;

    // 256 - resource header
    // 4 - length of icns data
    // icns_data.len() - icns data
    // 28 - resource map header
    // 2 - number of *different* resource TYPES (not total number of resources in the file)
    // 8 - a resource TYPE description
    // 12 - a resource description
    let size = 256 + (4 + icns_data.len()) + 28 + 2 + 8 + 12;
    let mut fork = vec![b'\0'; size];
    let slice = fork.as_mut_slice();
    let mut offset = 0;
    let mut ss = &mut slice[0..];

    // Resource file header, found at the start of the resource file.
    // 4 bytes: Offset from beginning of resource file to resource data. Basically guaranteed to be 0x100.
    BigEndian::write_u32(&mut ss[0..4], 256);

    // 4 bytes: Offset from beginning of resource file to resource map.
    let data_len = icns_data.len() + 4;
    BigEndian::write_u32(&mut ss[4..8], (data_len + 256) as u32);

    // 4 bytes: Length of resource data.
    BigEndian::write_u32(&mut ss[8..12], data_len as u32);

    // 4 bytes: Length of resource map.
    // for a single item, it's 50 bytes
    BigEndian::write_u32(&mut ss[12..16], 50);

    // 112 bytes: System-reserved data. In practice, this is usually all null bytes.
    ss[16..128].copy_from_slice(&[b'\0'; 112]);

    // 128 bytes: Application-specific data. In practice, this is usually all null bytes.
    ss[128..256].copy_from_slice(&[b'\0'; 128]);

    // icon data
    offset += 256;
    ss = &mut slice[offset..];
    BigEndian::write_u32(&mut ss[0..4], icns_data.len() as u32);
    ss[4..icns_data.len() + 4].copy_from_slice(&icns_data);

    // Header for the resource map, found immediately after the last resource data block. This position is also
    // indicated in the header.
    // # 16 bytes: Reserved for copy of resource header (in memory). Should be 0 in the file.
    // # 4 bytes: Reserved for handle to next resource map to be searched (in memory). Should be 0 in file.
    // # 2 bytes: Reserved for file reference number (in memory). Should be 0 in file.
    // # 2 bytes: Resource file attributes. Combination of ResourceFileAttrs flags, see below.
    // 2 bytes: Offset from beginning of resource map to type list.
    offset += 4 + icns_data.len();
    let rmap_offset = offset;
    ss = &mut slice[offset..];

    BigEndian::write_u16(&mut ss[24..26], 28);

    // 2 bytes: Offset from beginning of resource map to resource name list.
    BigEndian::write_u16(&mut ss[26..28], 50);

    // Header for the type list, found immediately after the resource map header.
    // 2 bytes: Number of resource types in the map minus 1
    offset += 28;
    ss = &mut slice[offset..];
    BigEndian::write_u16(&mut ss[0..2], 0);

    // A single type in the type list.
    offset += 2;
    ss = &mut slice[offset..];
    // 4 bytes: Resource type. This is usually a 4-character ASCII mnemonic, but may be any 4 bytes.
    ss[0..4].copy_from_slice(b"icns");
    // 2 bytes: Number of resources of this type in the map minus 1.
    BigEndian::write_u16(&mut ss[4..6], 0);

    // 2 bytes: Offset from beginning of type list to reference list for resources of this type.
    // I think this will always be 10 for a single resource type because
    BigEndian::write_u16(&mut ss[6..8], 10);

    // A single resource reference in a reference list. (A reference list has no header, and neither does the list of reference lists.)
    //
    // 1 byte: Resource attributes. Combination of ResourceAttrs flags, see below. (Note: packed into 4 bytes together with the next 3 bytes.)
    // 3 bytes: Offset from beginning of resource data to length of data for this resource. (Note: packed into 4 bytes together with the previous 1 byte.)
    // 4 bytes: Reserved for handle to resource (in memory). Should be 0 in file.
    offset += 8;
    ss = &mut slice[offset..];

    // 2 bytes: Resource ID.
    BigEndian::write_i16(&mut ss[0..2], -16455);
    // 2 bytes: Offset from beginning of resource name list to length of resource name, or -1 (0xffff) if none.
    BigEndian::write_u16(&mut ss[2..4], 0xffff);

    // copy of the resource header, it's weird
    slice.copy_within(0..16, rmap_offset);

    Ok(fork)
}

pub fn icon_apple_double(icns: &Path) -> std::io::Result<Vec<u8>> {
    let icns_data = generate_icns_fork(icns)?;
    let finder_data_len = 3760;

    // 26 - header
    // 24 - entry list description
    // 3760 - finder data
    // icns_data.len() - icon data
    let size = 26 + 24 + finder_data_len + icns_data.len();

    let mut data = vec![b'\0'; size];
    let slice = data.as_mut_slice();
    let mut offset = 0;
    let mut ss = &mut slice[offset..];

    // apple_double
    BigEndian::write_u32(&mut ss[0..4], 333319);

    // version. this was straight copied from an existing icon file
    BigEndian::write_u32(&mut ss[4..8], 131072);

    // unknown, but it exists in a real ._Icon\r file
    ss[8..24].copy_from_slice(b"Mac OS X        ");

    // number of entries, 2 for com.apple.FinderInfo and com.apple.ResourceFork
    BigEndian::write_u16(&mut ss[24..26], 2);

    offset += 26;
    ss = &mut slice[offset..];

    // 9 - finder info
    BigEndian::write_u32(&mut ss[0..4], 9);
    // body offset
    BigEndian::write_u32(&mut ss[4..8], 50);
    // body length
    BigEndian::write_u32(&mut ss[8..12], finder_data_len as u32);

    offset += 12;
    ss = &mut slice[offset..];

    // 2 - resource fork
    BigEndian::write_u32(&mut ss[0..4], 2);
    // body offset
    BigEndian::write_u32(&mut ss[4..8], (finder_data_len + 50) as u32);
    // body length
    BigEndian::write_u32(&mut ss[8..12], icns_data.len() as u32);

    offset += 12;
    ss = &mut slice[offset..];

    // finder data
    // file type
    ss[0..4].copy_from_slice(b"icon");
    // file creator
    ss[4..8].copy_from_slice(b"MACS");
    // flags
    BigEndian::write_u16(&mut ss[8..10], 16400);
    // location x
    BigEndian::write_u16(&mut ss[10..12], 0);
    // location y
    BigEndian::write_u16(&mut ss[12..14], 0);
    // folder id
    BigEndian::write_u16(&mut ss[14..16], 0);

    offset += finder_data_len;
    ss = &mut slice[offset..];

    ss[0..icns_data.len()].copy_from_slice(&icns_data);

    Ok(data)
}

pub fn folder_apple_double() -> std::io::Result<Vec<u8>> {
    let finder_data_len = 3760;
    let resource_len = 286;

    // 26 - header
    // 24 - entry list description
    // 3760 - finder data
    // 286
    let size = 26 + 24 + finder_data_len + resource_len;

    let mut data = vec![b'\0'; size];
    let slice = data.as_mut_slice();
    let mut offset = 0;
    let mut ss = &mut slice[offset..];

    // apple_double
    BigEndian::write_u32(&mut ss[0..4], 333319);

    // version. this was straight copied from an existing icon file
    BigEndian::write_u32(&mut ss[4..8], 131072);

    // unknown, but it exists in a real ._Icon\r file
    ss[8..24].copy_from_slice(b"Mac OS X        ");

    // number of entries, 2 for com.apple.FinderInfo and com.apple.ResourceFork
    BigEndian::write_u16(&mut ss[24..26], 2);

    offset += 26;
    ss = &mut slice[offset..];

    // 9 - finder info
    BigEndian::write_u32(&mut ss[0..4], 9);
    // body offset
    BigEndian::write_u32(&mut ss[4..8], 50);
    // body length
    BigEndian::write_u32(&mut ss[8..12], finder_data_len as u32);

    offset += 12;
    ss = &mut slice[offset..];

    // 2 - resource fork
    BigEndian::write_u32(&mut ss[0..4], 2);
    // body offset
    BigEndian::write_u32(&mut ss[4..8], (finder_data_len + 50) as u32);
    // body length
    BigEndian::write_u32(&mut ss[8..12], resource_len as u32);

    offset += 12;
    ss = &mut slice[offset..];

    // finder data
    // file type
    ss[0..4].copy_from_slice(b"icon");
    // file creator
    ss[4..8].copy_from_slice(b"MACS");
    // flags
    BigEndian::write_u16(&mut ss[8..10], 16400);
    // location x
    BigEndian::write_u16(&mut ss[10..12], 0);
    // location y
    BigEndian::write_u16(&mut ss[12..14], 0);
    // folder id
    BigEndian::write_u16(&mut ss[14..16], 0);

    // commented out to suppress warning #[warn(unused_mut)] on mac. but these were uncommented. the line after these
    // 2 were not, however
    //offset += finder_data_len;
    //ss = &mut slice[offset..];

    //ss[0..icns_data.len()].copy_from_slice(&icns_data);

    Ok(data)
}
