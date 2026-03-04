/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#f2f8ff",
          100: "#d9e9ff",
          200: "#b9d7ff",
          300: "#87bbff",
          400: "#509bff",
          500: "#2b7df6",
          600: "#1e61d1",
          700: "#1c4ca5",
          800: "#1c4288",
          900: "#1b396f"
        },
        ink: "#0f172a",
        paper: "#f8fafc"
      },
      fontFamily: {
        sans: ['"Plus Jakarta Sans"', "ui-sans-serif", "system-ui"],
        mono: ['"IBM Plex Mono"', "ui-monospace", "SFMono-Regular"]
      }
    }
  },
  plugins: []
};
