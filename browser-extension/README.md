# Lumina Privacy Tools

Extensión independiente para Chrome, Edge y otros navegadores compatibles con Manifest V3.

## Instalación local

1. Abre `chrome://extensions` o `edge://extensions`.
2. Activa **Developer mode / Modo desarrollador**.
3. Pulsa **Load unpacked / Cargar descomprimida**.
4. Selecciona esta carpeta `browser-extension`.
5. Fija Lumina en la barra del navegador.
6. Usa el botón de la extensión para activar o desactivar la protección.

La lista incluida bloquea algunos dominios conocidos de anuncios y analítica. No pretende sustituir EasyList, uBlock Origin ni una auditoría de cada página. Debe ampliarse y probarse antes de distribuirse públicamente.

Desde el popup también puedes escribir un dominio personalizado, bloquearlo y quitarlo después. La regla se guarda en el almacenamiento local de la extensión y se aplica con `declarativeNetRequest`.
