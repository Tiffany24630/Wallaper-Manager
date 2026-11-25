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

Abre Docker Desktop y espera a que el motor Linux esté listo. Desde la raíz del proyecto:

```powershell
docker info
docker compose config --quiet
docker compose up --build --abort-on-container-exit --exit-code-from lumina-build
```

El servicio `lumina-build` compila el proyecto, verifica que existan el frontend generado y el ejecutable Linux, y termina. **`Exited (0)` es el resultado esperado**, no un fallo: no hay un servidor ni una interfaz gráfica que deban permanecer ejecutándose. Consulta el resultado con `docker compose ps -a` y `docker compose logs lumina-build`.

Para ejecutar también las pruebas de Rust dentro del entorno de compilación:

```powershell
docker compose run --rm lumina-build cargo test --locked
```

Si `docker info` muestra `dockerDesktopLinuxEngine` junto con `The system cannot find the file specified`, el CLI no puede conectar con el motor de Docker Desktop. Inicia Docker Desktop, comprueba que use contenedores Linux y vuelve a ejecutar `docker info` antes de compilar. No es necesario borrar imágenes ni volúmenes.

Como alternativa, para construir únicamente la imagen de artefactos (sin un comando de ejecución):

```powershell
docker build --tag lumina-checks .
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

## Publicación en Vercel, Render y Neon

Arquitectura gratuita: Vercel sirve `frontend/dist`; Render ejecuta `backend/server.py`; Neon guarda cuentas, sesiones y metadatos. Render Free usa almacenamiento efímero para los archivos sincronizados, por lo que esos archivos pueden desaparecer al reiniciar o redeployar el servicio. El escritorio actual usa Tauri 2, no Electron, y conserva SQLite local para operaciones nativas.

### Neon

1. Crea un proyecto y una base de datos en Neon.
2. En **Connect**, selecciona **Python / psycopg** y copia la cadena pooled `postgresql://...` con `sslmode=require`.
3. No la guardes en Git: se pegará como secreto en Render.

### Render

En Render selecciona **New > Web Service**; no uses **Blueprint**. Completa **Name** `lumina-api`, **Runtime** `Python 3`, **Root Directory** `backend`, **Build Command** `pip install -r requirements.txt`, **Start Command** `python server.py`, **Health Check Path** `/health` y el plan **Free**. No agregues Persistent Disk: no está disponible en el plan gratuito.

En **Environment Variables** agrega `DATABASE_URL` = cadena pooled de Neon; `LUMINA_API_ALLOWED_ORIGINS` = `https://TU-PROYECTO.vercel.app` (separados por comas, nunca `*`); `LUMINA_API_STORAGE` = `lumina-files`; `LUMINA_MAX_FILE_BYTES` = `104857600`. Comprueba `https://TU-SERVICIO.onrender.com/health`.

El plan Free sirve para probar la API y sincronizar metadatos, pero no garantiza la conservación de imágenes, cursores u otros archivos después de reinicios o despliegues. Para persistencia gratuita de archivos hace falta añadir posteriormente un almacenamiento de objetos externo; Neon no debe usarse como almacén de blobs grandes.

Para persistir archivos sin disco de Render puedes configurar cualquier almacenamiento S3-compatible. En Render agrega `LUMINA_S3_BUCKET`, `LUMINA_S3_ENDPOINT_URL`, `LUMINA_S3_REGION`, `LUMINA_S3_ACCESS_KEY_ID` y `LUMINA_S3_SECRET_ACCESS_KEY`. Para Cloudflare R2, `LUMINA_S3_ENDPOINT_URL` es `https://ACCOUNT_ID.r2.cloudflarestorage.com` y la región es `auto`. El backend sube cada archivo al bucket y, si el archivo local ya no existe después de un reinicio, lo sirve desde el bucket.

### Vercel

1. **Add New > Project** e importa el repositorio.
2. **Framework Preset:** `Vite`; **Root Directory:** `frontend`.
3. **Build Command:** `npm run typecheck && npm run build`; **Output Directory:** `dist`; **Install Command:** `npm ci` (la configuración está en `frontend/vercel.json`).
4. **Environment Variables:** `VITE_LUMINA_API_URL` = `https://TU-SERVICIO.onrender.com` para Production, Preview y Development.
5. Pulsa **Deploy** y actualiza `LUMINA_API_ALLOWED_ORIGINS` en Render con el dominio Vercel definitivo.

### Windows/Tauri

Instala Node 20+, Rust, WebView2 y Visual Studio Build Tools con **Desktop development with C++**. Ejecuta `cd frontend`, `npm ci`, `npm run typecheck`, `npm run build` y `npm run desktop:build`. En **Cuenta > URL de la API** coloca `https://TU-SERVICIO.onrender.com`, guarda, inicia sesión y vincula el equipo. Los instaladores quedan en `src-tauri/target/release/bundle`.

### Android

Abre Android Studio con JDK 17 y SDK 35. Ejecuta `gradlew.bat :app:assembleDebug`, o genera el `.aab` firmado desde **Build > Generate Signed App Bundle / APK**. En la URL del servidor coloca `https://TU-SERVICIO.onrender.com`; nunca HTTP en producción. Para Play Store incrementa `versionCode` y conserva la clave de firma.

### Validación y seguridad

```powershell
python -m unittest discover -s backend -p 'test*.py'
cd frontend
npm ci
npm run typecheck
npm run build
```

Prueba `/health`, registro, login, vinculación, carga/descarga de un PNG y revocación. No publiques `DATABASE_URL`, tokens, keystores Android ni certificados de firma. La web usa `sessionStorage` para su token; Tauri usa el almacén seguro de Windows. Las funciones de disco, monitores, cursores y Real-ESRGAN son nativas y no se simulan en Vercel.

## Extensión de privacidad del navegador

La web no puede bloquear anuncios en otras pestañas. Para esa capacidad se añadió `browser-extension/`, una extensión Manifest V3 independiente para Chrome, Edge y navegadores compatibles.

1. Abre `chrome://extensions` o `edge://extensions`.
2. Activa **Developer mode / Modo desarrollador**.
3. Pulsa **Load unpacked / Cargar descomprimida**.
4. Selecciona la carpeta `browser-extension`.
5. Fija `Lumina Privacy Tools` y usa su interruptor.

La lista inicial bloquea dominios publicitarios y de analítica conocidos. Antes de distribuirla públicamente hay que ampliar las listas, probar falsos positivos y preparar las políticas de privacidad y permisos de la tienda.

Las preferencias de inicio automático, bandeja y ahorro contextual se almacenan para una futura integración residente. La interfaz las identifica como preferencias y no simula que ya estén activas.
