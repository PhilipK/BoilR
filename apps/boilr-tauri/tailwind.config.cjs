// BoilR's palette is the 8-colour pixel sunset the original egui app used.
module.exports = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    colors: {
      transparent: "transparent",
      current: "currentColor",
      night: "#0D2B45",
      deep: "#0A1E3C",
      harbour: "#203C56",
      dusk: "#544E68",
      mauve: "#8D697A",
      ember: "#D08159",
      flame: "#FFAA5E",
      peach: "#FFD4A3",
      foam: "#FFECD6",
    },
    fontFamily: {
      pixel: ['"Pixelify Sans"', "monospace"],
      sans: ['"Atkinson Hyperlegible Next"', "system-ui", "sans-serif"],
    },
    extend: {
      borderWidth: { 3: "3px" },
    },
  },
  plugins: [require("@tailwindcss/forms")],
};
