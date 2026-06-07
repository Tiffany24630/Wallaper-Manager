# Lumina Backend

Backend de escritorio para Wallaper Manager construido con Tauri 2.

## Funcionalidad

- Escanea `backend/wallpapers/images`, `videos` y `animated`.
- Importa archivos locales a la biblioteca.
- Guarda favoritos y wallpaper activo en `backend/config/state.json`.
- Guarda playlists y preferencias de la app en `backend/config/state.json`.
- Aplica imagenes estaticas al escritorio en Windows.
- Carga el frontend React compilado desde `frontend/dist`.

## Comandos Tauri

- `get_backend_status`
- `list_wallpapers`
- `import_wallpaper`
- `set_active_wallpaper`
- `toggle_favorite`
- `list_playlists`
- `create_playlist`
- `delete_playlist`
- `add_wallpaper_to_playlist`
- `get_settings`
- `save_settings`

## Desarrollo

Compila el frontend y luego ejecuta Tauri desde `backend/src-tauri`:

```bash
cd frontend
npm run build
cd ../backend/src-tauri
cargo run
```

Si solo cambias Rust despues de compilar el frontend una vez, puedes volver a ejecutar:

```bash
cargo run
```
