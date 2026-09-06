# Lumina Wallpaper Manager

Aplicación de escritorio para Windows construida con React, TypeScript, Tauri y Rust. Administra una biblioteca local de fondos, listas, favoritos, múltiples pantallas, métricas reales del dispositivo y mejora de resolución con Real-ESRGAN.

## Requisitos

- Node.js 20 o superior
- Rust estable y Cargo
- Visual Studio Build Tools con el conjunto **Desktop development with C++**
- WebView2 (incluido en Windows 11)
- Una GPU con Vulkan para la mejora mediante Real-ESRGAN

## Desarrollo

```powershell
cd frontend
npm ci
npm run desktop:dev
```

La base de datos, los originales administrados y las miniaturas se guardan en el directorio de datos de la aplicación que proporciona Windows. Las imágenes no se suben a ningún servidor.

## Comprobaciones

```powershell
cd frontend
npm run typecheck
npm run build
npm run desktop:build
```

## Funciones

- Importación validada de PNG, JPG, WEBP y BMP, con detección de duplicados por SHA-256.
- Metadatos reales de resolución y tamaño, miniaturas, favoritos y aplicación del fondo.
- Listas persistentes y asignación de fondos por monitor.
- CPU, RAM, almacenamiento, memoria del proceso e historial de uso obtenidos localmente.
- Escalado local 3× o 4× con los binarios y modelos incluidos de Real-ESRGAN.
- Preferencias persistentes en SQLite.

Las preferencias de inicio automático, bandeja y ahorro contextual se almacenan, pero requieren un servicio residente futuro para actuar cuando la ventana no esté abierta. La interfaz las identifica como preferencias y no simula que la integración ya está activa.
