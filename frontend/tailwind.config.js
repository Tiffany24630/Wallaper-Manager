/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        ink: {
          950: "#080b13",
          900: "#0f172a",
          800: "#111827",
        },
        cyanfire: "#22d3ee",
        mint: "#34d399",
        amberlight: "#fbbf24",
      },
      boxShadow: {
        panel: "0 24px 80px rgba(2, 8, 23, 0.32)",
        glow: "0 0 32px rgba(34, 211, 238, 0.22)",
      },
    },
  },
  plugins: [],
}
