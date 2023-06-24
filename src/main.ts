import type { Plugin, ViteDevServer, UserConfig } from "vite";

import { get_macro_locations, initSync } from "./wasm/vite_plugin_macro";
import fs from "node:fs";
import url from "node:url";
import path from "node:path";
import MagicString from "magic-string";
import { ViteNodeRunner } from "vite-node/client";
import { ViteNodeServer } from "vite-node/server";
import { createFilter } from "@rollup/pluginutils";
import type { FilterPattern } from "@rollup/pluginutils";

export interface MacroReplaceLocation {
  lo: number;
  hi: number;
  import_src: string;
  import_name: string;
}

export interface MacroRemoveLocation {
  lo: number;
  hi: number;
}

export interface MacroOutput {
  replaces: MacroReplaceLocation[];
  removals: MacroRemoveLocation[];
}

export interface MacroPluginOptions {
  filter?: (ident: string, id: string, importer: string) => boolean;
  assertType?: string;
  include?: FilterPattern | undefined;
  exclude?: FilterPattern | undefined;
  rootDir?: string;
}

interface Runner {
  executeId(rawId: string): Promise<any>;
}

export const macroPlugin = async (
  opts: MacroPluginOptions = {}
): Promise<Plugin> => {
  const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
  const wasmBuffer = await fs.promises.readFile(
    path.join(__dirname, "vite_plugin_macro_bg.wasm")
  );
  const assertType = opts.assertType ?? "";
  const filter = opts.filter ? opts.filter : () => false;
  const idFilter = createFilter(
    opts.include,
    opts.exclude ?? [/\bnode_modules\b/],
    { resolve: false }
  );
  initSync(wasmBuffer);
  const rootDir = opts.rootDir ? opts.rootDir : process.cwd();
  let server: ViteDevServer | undefined;
  let runner: Runner;
  let config: Readonly<
    Omit<UserConfig, "plugins" | "assetsInclude" | "optimizeDeps" | "worker">
  >;
  return {
    name: "vite-plugin-macro",
    enforce: "pre",
    configResolved(c) {
      config = c;
    },
    configureServer(s) {
      server = s;
    },
    buildStart: {
      sequential: true,
      async handler() {
        if (server) {
          const node = new ViteNodeServer(server);
          runner = new ViteNodeRunner({
            root: server.config.root,
            base: server.config.base,
            fetchModule(id) {
              return node.fetchModule(id);
            },
            resolveId: (id, importer) => {
              return node.resolveId(id, importer);
            },
          });
        } else {
          runner = {
            async executeId(id) {
              return import(id);
            },
          };
        }
      },
    },
    async transform(code, id) {
      if (id.startsWith("\0")) {
        return;
      }
      if (!id.startsWith(rootDir)) {
        return;
      }
      if (!idFilter(id)) {
        return;
      }
      const extension = path.extname(id);
      const shouldTransform = [".js", ".jsx", ".ts", ".tsx"].includes(
        extension
      );
      if (!shouldTransform) {
        return;
      }
      const value = get_macro_locations(
        code,
        id,
        assertType,
        (ident: string, source: string) => filter(ident, source, id)
      ) as MacroOutput;
      const s = new MagicString(code);
      for (const macroLocation of value.removals) {
        s.remove(macroLocation.lo, macroLocation.hi);
      }
      for (const macroLocation of value.replaces) {
        const resolved = await this.resolve(macroLocation.import_src, id);
        if (resolved && resolved.id) {
          const module = await runner.executeId(resolved.id);
          const macroFunc = module[macroLocation.import_name];
          if (macroFunc) {
            const wrapperStr =
              "return " + s.slice(macroLocation.lo, macroLocation.hi);
            const macroWrapper = new Function(
              macroLocation.import_name,
              wrapperStr
            );
            const result = macroWrapper(macroFunc);
            s.remove(macroLocation.lo, macroLocation.hi);
            s.appendLeft(macroLocation.lo, JSON.stringify(result));
          }
        }
      }
      return {
        code: s.toString(),
        map: s.generateMap({ hires: true }),
      };
    },
  };
};
