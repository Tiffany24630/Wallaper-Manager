use image::imageops::FilterType;

pub fn generate_thumbnail(
    source: &str,
    output: &str,
) -> Result<(), String> {
    let image = image::open(source).map_err(|e| e.to_string())?;
    let thumb = image.resize(400, 250, FilterType::Lanczos3,);
    
    thumb.save(output).map_err(|e| e.to_string())?;

    Ok(())
}