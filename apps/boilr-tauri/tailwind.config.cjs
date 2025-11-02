module.exports = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        slate: {
          950: "#0e1118"
        }
      }
    }
  },
  plugins: [require("@tailwindcss/forms")],
};
