use exe::VecPE;
use exe::types::{ResolvedDirectoryID, ResourceDirectory, ResourceID};

struct IconPart {
    width: u8,
    height: u8,
    color_count: u8,
    planes: u16,
    bit_count: u16,
    data: Vec<u8>,
}

fn build_ico_from_group(
    image: &VecPE,
    group: &exe::headers::GrpIconDir,
    resource_dir: &ResourceDirectory,
) -> Option<Vec<u8>> {
    let count = group.count as usize;
    if count == 0 {
        return None;
    }

    let mut parts: Vec<IconPart> = Vec::with_capacity(count);

    for entry in group.entries.iter() {
        let id = ResolvedDirectoryID::ID(entry.id as u32);
        let matches = resource_dir.filter(
            Some(ResolvedDirectoryID::ID(ResourceID::Icon as u32)),
            Some(id),
            None,
        );
        let data_entry = matches.first()?.get_data_entry(image).ok()?;
        let raw = data_entry.read(image).ok()?;
        parts.push(IconPart {
            width: entry.width,
            height: entry.height,
            color_count: entry.color_count,
            planes: entry.planes,
            bit_count: entry.bit_count,
            data: raw.to_vec(),
        });
    }

    build_ico_buffer(&parts)
}

/// Assemble an ICO file from icon parts: a 6-byte `ICONDIR`, one 16-byte
/// `ICONDIRENTRY` per part, then the concatenated image data.
///
/// Width/height bytes are passed through verbatim — GRP entries and ICO
/// entries share the same encoding (0 means 256), so no remapping is done.
fn build_ico_buffer(parts: &[IconPart]) -> Option<Vec<u8>> {
    if parts.is_empty() {
        return None;
    }
    let count = parts.len();
    let header_size = 6 + 16 * count;
    let mut data_offset = header_size;
    let total = header_size + parts.iter().map(|p| p.data.len()).sum::<usize>();
    let mut buf = Vec::with_capacity(total);

    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&(count as u16).to_le_bytes());

    for part in parts {
        buf.push(part.width);
        buf.push(part.height);
        buf.push(part.color_count);
        buf.push(0u8);
        buf.extend_from_slice(&part.planes.to_le_bytes());
        buf.extend_from_slice(&part.bit_count.to_le_bytes());
        buf.extend_from_slice(&(part.data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(data_offset as u32).to_le_bytes());
        data_offset += part.data.len();
    }

    for part in parts {
        buf.extend_from_slice(&part.data);
    }
    Some(buf)
}

pub fn extract_icon(image: &VecPE) -> Option<Vec<u8>> {
    let resource_dir = ResourceDirectory::parse(image).ok()?;
    let icon_groups = resource_dir.icon_groups(image).ok()?;
    let best_group = icon_groups
        .values()
        .max_by_key(|g| g.entries.iter().map(|e| e.bytes_in_res as u64).sum::<u64>())?;
    let ico_data = build_ico_from_group(image, best_group, &resource_dir)?;
    let img = image::load_from_memory(&ico_data).ok()?;
    let img = img.resize(128, 128, image::imageops::FilterType::Lanczos3);
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(width: u8, height: u8, data: &[u8]) -> IconPart {
        IconPart {
            width,
            height,
            color_count: 0,
            planes: 1,
            bit_count: 32,
            data: data.to_vec(),
        }
    }

    #[test]
    fn empty_parts_returns_none() {
        assert!(build_ico_buffer(&[]).is_none());
    }

    #[test]
    fn single_part_ico_layout() {
        let parts = [part(32, 32, &[0xAA, 0xBB, 0xCC, 0xDD])];
        let ico = build_ico_buffer(&parts).unwrap();

        assert_eq!(ico.len(), 6 + 16 + 4);

        assert_eq!(&ico[0..2], &0u16.to_le_bytes());
        assert_eq!(&ico[2..4], &1u16.to_le_bytes());
        assert_eq!(&ico[4..6], &1u16.to_le_bytes());

        assert_eq!(ico[6], 32);
        assert_eq!(ico[7], 32);
        assert_eq!(ico[8], 0);
        assert_eq!(ico[9], 0);
        assert_eq!(&ico[10..12], &1u16.to_le_bytes());
        assert_eq!(&ico[12..14], &32u16.to_le_bytes());
        assert_eq!(&ico[14..18], &4u32.to_le_bytes());
        assert_eq!(&ico[18..22], &22u32.to_le_bytes());

        assert_eq!(&ico[22..], &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn multiple_parts_have_contiguous_offsets() {
        let first = vec![0u8; 10];
        let second = vec![1u8; 20];
        let parts = [part(16, 16, &first), part(48, 48, &second)];
        let ico = build_ico_buffer(&parts).unwrap();

        assert_eq!(&ico[4..6], &2u16.to_le_bytes());
        let data_start = 6 + 16 * 2;
        assert_eq!(ico.len(), data_start + 30);

        assert_eq!(&ico[6..8], &[16, 16]);
        assert_eq!(&ico[14..18], &10u32.to_le_bytes());
        assert_eq!(&ico[18..22], &(data_start as u32).to_le_bytes());

        assert_eq!(&ico[22..24], &[48, 48]);
        assert_eq!(&ico[30..34], &20u32.to_le_bytes());
        assert_eq!(&ico[34..38], &((data_start + 10) as u32).to_le_bytes());

        assert_eq!(&ico[data_start..data_start + 10], &first[..]);
        assert_eq!(&ico[data_start + 10..], &second[..]);
    }

    #[test]
    fn width_height_255_is_passed_through_verbatim() {
        let parts = [part(255, 255, &[0x00])];
        let ico = build_ico_buffer(&parts).unwrap();
        assert_eq!(ico[6], 255);
        assert_eq!(ico[7], 255);
    }

    #[test]
    fn width_height_zero_is_preserved() {
        let parts = [part(0, 0, &[0x00])];
        let ico = build_ico_buffer(&parts).unwrap();
        assert_eq!(&ico[6..8], &[0, 0]);
    }
}
