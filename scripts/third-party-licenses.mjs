// Genera THIRD_PARTY_LICENSES.md con los componentes de terceros que se
// distribuyen con Nebula Reader:
// - crates de Rust enlazados en el ejecutable (`cargo tree -e normal`);
// - paquetes de JavaScript incluidos en la interfaz (y sus dependencias);
// - PDFium y UnRAR, con el texto completo de su licencia.
//
// Uso: npm run licenses   (requiere haber ejecutado antes scripts\fetch-pdfium.ps1)
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const run = (cmd, args) =>
  execFileSync(cmd, args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

// ---------- Rust ----------
const tree = run("cargo", [
  "tree", "-p", "nebula-reader", "-e", "normal", "--target", "x86_64-pc-windows-msvc",
  "--prefix", "none", "--format", "{p}|{l}|{r}",
]);
const crates = new Map();
for (const line of tree.split(/\r?\n/)) {
  const [pkg, license, repo] = line.replace(/ \(\*\)$/, "").split("|");
  if (!pkg || pkg.startsWith("nebula-")) continue;
  const [name, version] = pkg.trim().split(" ");
  crates.set(`${name}@${version}`, { name, version: version.replace(/^v/, ""), license: license || "?", repo: repo || "" });
}

const metadata = JSON.parse(run("cargo", ["metadata", "--format-version", "1", "--locked"]));
const manifestDir = (name) => {
  const pkg = metadata.packages.find((p) => p.name === name);
  return pkg ? dirname(pkg.manifest_path) : null;
};

// ---------- JavaScript ----------
const npmRoots = ["svelte", "@sveltejs/kit", "@tauri-apps/api", "@tauri-apps/plugin-dialog"];
const npm = new Map();
const visit = (name) => {
  if (npm.has(name)) return;
  const file = join(root, "node_modules", name, "package.json");
  if (!existsSync(file)) return;
  const pkg = JSON.parse(readFileSync(file, "utf8"));
  const repo = typeof pkg.repository === "string" ? pkg.repository : (pkg.repository?.url ?? "");
  npm.set(name, { name, version: pkg.version, license: pkg.license ?? "?", repo: repo.replace(/^git\+/, "") });
  for (const dep of Object.keys(pkg.dependencies ?? {})) visit(dep);
};
npmRoots.forEach(visit);

// ---------- Documento ----------
const table = (items) =>
  [
    "| Componente | Versión | Licencia | Origen |",
    "|---|---|---|---|",
    ...[...items]
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((c) => `| ${c.name} | ${c.version} | ${c.license} | ${c.repo} |`),
  ].join("\n");

const fullText = (title, path) =>
  existsSync(path)
    ? `## ${title}\n\n\`\`\`text\n${readFileSync(path, "utf8").trim()}\n\`\`\`\n`
    : (() => {
        throw new Error(`No se encuentra ${path}`);
      })();

const unrarDir = manifestDir("unrar_sys");
const doc = `# Licencias de terceros

Nebula Reader (licencia MIT) incluye o enlaza los componentes de terceros
siguientes. Archivo generado con \`npm run licenses\`; no editar a mano.

Las licencias MPL-2.0 que aparecen son de dependencias de Tauri; su código
fuente está disponible públicamente en sus repositorios.

## Componentes nativos con licencia propia

- **PDFium** (lectura de PDF), binarios de https://github.com/bblanchon/pdfium-binaries — texto completo más abajo.
- **UnRAR** (lectura de CBR), de RARLAB, incluido a través del crate \`unrar_sys\` — texto completo más abajo.
  Permite usar el código para extraer archivos RAR, pero no para recrear el algoritmo de compresión RAR.

## Crates de Rust (${crates.size})

${table(crates.values())}

## Paquetes de JavaScript (${npm.size})

${table(npm.values())}

${fullText("Licencia de PDFium", join(root, "src-tauri", "resources", "pdfium", "LICENSE"))}
${fullText("Licencia de UnRAR", join(unrarDir ?? "", "vendor", "unrar", "license.txt"))}`;

writeFileSync(join(root, "THIRD_PARTY_LICENSES.md"), doc);
console.log(`THIRD_PARTY_LICENSES.md: ${crates.size} crates, ${npm.size} paquetes npm`);
