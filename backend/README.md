# Lumina Backend

Backend de escritorio para Wallaper Manager construido con Tauri 2.

## Funcionalidad

- Escanea `backend/wallpapers/images`, `videos` y `animated`.
- Importa archivos locales a la biblioteca.
- Guarda favoritos y wallpaper activo en `backend/config/state.json`.
- Aplica imagenes estaticas al escritorio en Windows.

## Comandos Tauri

- `get_backend_status`
- `list_wallpapers`
- `import_wallpaper`
- `set_active_wallpaper`
- `toggle_favorite`

## Desarrollo

Ejecuta desde `backend/src-tauri`:

```bash
cargo run
```
