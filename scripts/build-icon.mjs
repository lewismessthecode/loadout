import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync } from "node:fs";

const svg = readFileSync("design/icon.svg", "utf8");
const png = new Resvg(svg, { fitTo: { mode: "width", value: 1024 } }).render().asPng();
writeFileSync("design/icon-1024.png", png);
console.log(`wrote design/icon-1024.png (${png.length} bytes)`);
