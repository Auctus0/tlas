// Vite `define` ile package.json sürümü derleme sırasında gömülür (vite.config.js).
declare global {
  const __APP_VERSION__: string;
}

export {};
