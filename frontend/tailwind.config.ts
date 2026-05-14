import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}"
  ],

  theme: {
    extend: {
      colors: {
        background: "#332063",
        surface: "#4C4C9C",
        card: "#5E3AA2",
        primary: "#7559CB",
        secondary: "#9563DE",
        accent: "#997BF8",
        soft: "#D4A7F9",
        text: "#FBDBFB",
        muted: "#B6909E",
        bronze: "#8F716A"
      },

      boxShadow: {
        glow: "0 0 25px rgba(149, 99, 222, 0.45)"
      },

      borderRadius: {
        xl2: "1.25rem",
        xl3: "1.75rem"
      },

      backgroundImage: {
        mainGradient:
          "linear-gradient(135deg, #332063 0%, #4C4C9C 40%, #7559CB 100%)",

        cardGradient:
          "linear-gradient(145deg, #5E3AA2 0%, #4C4C9C 100%)"
      }
    }
  },

  plugins: []
};

export default config;