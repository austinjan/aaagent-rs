/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // BlackBear TechHive brand colors
        'brand-black': '#000000',
        'brand-yellow': '#E8C236',
      },
    },
  },
  plugins: [require("daisyui")],
  daisyui: {
    themes: [
      {
        blackbear: {
          "primary": "#E8C236",      // Brand yellow
          "secondary": "#A7A8A9",     // Dark gray
          "accent": "#66CC33",        // Green
          "neutral": "#000000",       // Brand black
          "base-100": "#FFFFFF",      // White
          "base-200": "#DDE5ED",      // Blue gray
          "base-300": "#D7D2CB",      // Warm gray
          "info": "#33CCFF",          // Cyan
          "success": "#339933",       // Dark green
          "warning": "#EEC049",       // Yellow light
          "error": "#EB0FFF",         // Magenta
        },
      },
    ],
  },
};
