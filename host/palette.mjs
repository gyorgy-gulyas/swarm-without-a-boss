// Shared colours for the PNG snapshots and the browser viewer.
export const PALETTE = {
  bg: [8, 13, 26],          // ink blue-black: not measured (yet)
  once: [233, 224, 204],    // bone white: measured once
  dup: [214, 140, 50],      // amber: measured more than once
  rover: [255, 255, 255],
  measuring: [214, 140, 50],
  dead: [200, 70, 60],
  link: [110, 125, 190],
  lander: [214, 140, 50],
};

export const css = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;
