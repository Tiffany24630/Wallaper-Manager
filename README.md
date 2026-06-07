# Wallpaper Manager

Aplicación de escritorio desarrollada con React + Tauri para administrar fondos de pantalla locales.

## Requisitos

* Node.js 22+
* Rust
* Cargo
* Visual Studio Build Tools (Windows)

## Instalación

```bash
npm install
```

## Ejecutar en desarrollo

```bash
npm run tauri dev
```

## Compilar

```bash
npm run tauri build
```

## Funcionalidades

* Visualización de información real del sistema.
* Detección de monitores.
* Administración de fondos de pantalla.
* Favoritos.
* Playlists.
* Configuración persistente.
* Base de datos SQLite integrada.

## Base de Datos

La aplicación crea automáticamente:

```text
app.db
```

en:

```text
%APPDATA%/WallpaperManager
```

## Estructura

```text
src/
src-tauri/
assets/
wallpapers/
```

## Tecnologías

* React
* TypeScript
* Tauri
* SQLite
* Rust

```
```
