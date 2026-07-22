import path from "node:path"
import { fileURLToPath } from "node:url"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "astro/config"

const root = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  site: "https://ethira-dev.github.io",
  base: "/cli/",
  vite: {
    plugins: [tailwindcss()],
    resolve: {
      alias: {
        "@": path.join(root, "src"),
      },
    },
  },
})
