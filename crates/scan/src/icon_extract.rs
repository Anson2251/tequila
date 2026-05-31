use exe::VecPE;
use exe::types::{ResolvedDirectoryID, ResourceDirectory, ResourceID};

fn build_ico_from_group(
    image: &VecPE,
    group: &exe::headers::GrpIconDir,
    resource_dir: &ResourceDirectory,
) -> Option<Vec<u8>> {
    let count = group.count as usize;
    if count == 0 {
        return None;
    }

    struct IconPart {
        width: u8,
        height: u8,
        color_count: u8,
        planes: u16,
        bit_count: u16,
        data: Vec<u8>,
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

    if parts.is_empty() {
        return None;
    }
    let header_size = 6 + 16 * count;
    let mut data_offset = header_size;
    let total = header_size + parts.iter().map(|p| p.data.len()).sum::<usize>();
    let mut buf = Vec::with_capacity(total);

    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&(count as u16).to_le_bytes());

    for part in &parts {
        buf.push(if part.width >= 255 { 0 } else { part.width });
        buf.push(if part.height >= 255 { 0 } else { part.height });
        buf.push(part.color_count);
        buf.push(0u8);
        buf.extend_from_slice(&part.planes.to_le_bytes());
        buf.extend_from_slice(&part.bit_count.to_le_bytes());
        buf.extend_from_slice(&(part.data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(data_offset as u32).to_le_bytes());
        data_offset += part.data.len();
    }

    for part in &parts {
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
