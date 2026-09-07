# Lumina Wallpaper Manager

Aplicación de escritorio para Windows construida con React, TypeScript, Tauri y Rust. Administra una biblioteca local de fondos, listas, favoritos, múltiples pantallas, métricas reales del dispositivo y mejora local de imágenes.

## Funciones

- Importación validada de PNG, JPG, WEBP y BMP, con detección de duplicados por SHA-256.
- Metadatos reales de resolución y tamaño, miniaturas, favoritos y aplicación del fondo con el modo de ajuste seleccionado.
- Pregunta de mejora después de importar, escalado local 3× o 4× y comparación visual para conservar la original o la mejorada.
- Real-ESRGAN mediante Vulkan cuando está disponible y respaldo Lanczos cuando el equipo no puede ejecutar el modelo de IA.
- Listas persistentes y asignación de fondos por monitor.
- CPU, RAM, almacenamiento, memoria del proceso, monitores e historial de uso obtenidos localmente.
- Color de acento, escala de interfaz y preferencias persistentes en SQLite.

Las imágenes no se suben a servidores. La biblioteca, miniaturas y base de datos se guardan en el directorio de datos de la aplicación de Windows.

## Requisitos para Windows

- Node.js 20 o superior.
- Rust estable y Cargo.
- Visual Studio Build Tools con **Desktop development with C++**.
- WebView2 (incluido en Windows 11).
- Opcional: GPU con Vulkan para Real-ESRGAN. Sin Vulkan se usa el respaldo local.

## Desarrollo y aplicación de escritorio

```powershell
cd frontend
npm ci
npm run desktop:dev
```

Para generar el instalador y el ejecutable de escritorio:

```powershell
cd frontend
npm run typecheck
npm run build
npm run desktop:build
```

Los artefactos quedan en `src-tauri/target/release/bundle`.

## Compilación reproducible con Docker

```powershell
docker build --tag lumina-build .
```

La imagen compila el frontend de producción y una versión Linux del núcleo Tauri. Esto sirve como comprobación reproducible del proyecto. Docker no puede producir ni ejecutar correctamente una aplicación gráfica nativa de Windows; el instalador MSI/NSIS debe generarse con `npm run desktop:build` en Windows.

## Comprobaciones

```powershell
cd frontend
npm run typecheck
npm run build
cd ../src-tauri
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

Las preferencias de inicio automático, bandeja y ahorro contextual se almacenan para una futura integración residente. La interfaz las identifica como preferencias y no simula que ya estén activas.
