use image::imageops::FilterType;
use std::path::Path;

pub fn generate_thumbnail(source: &str, output: &str) -> Result<(), String> {
    let image = image::open(source)
        .map_err(|e| format!("No se pudo leer la imagen para la miniatura: {e}"))?;
    let thumb = image.resize(400, 250, FilterType::Lanczos3);

    thumb
        .save(output)
        .map_err(|e| format!("No se pudo guardar la miniatura: {e}"))?;
    if !Path::new(output).is_file() {
        return Err("La miniatura no se generó correctamente".into());
    }
    image::open(output).map_err(|e| format!("La miniatura generada no se puede leer: {e}"))?;

    Ok(())
}
