import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";

export default defineConfig({
  mode: "lib",
  build: {
    minify: false,
    lib: {
      entry: "src/main.ts",
      formats: ["es"],
      fileName: "index",
    },
    rollupOptions: {
      external: [
        "magic-string",
        "vite",
        "node:path",
        "node:fs",
        "node:url",
        "vite-node/server",
        "vite-node/client",
        "@rollup/pluginutils",
      ],
    },
  },
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: "src/wasm/vite_plugin_macro_bg.wasm",
          dest: ".",
        },
      ],
    }),
  ],
});
