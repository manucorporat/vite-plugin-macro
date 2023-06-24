import type { Plugin } from "vite";
import type { FilterPattern } from "@rollup/pluginutils";
export interface MacroPluginOptions {
  filter?: (id: string, source: string) => boolean;
  assertType?: string;
  include?: FilterPattern | undefined;
  exclude?: FilterPattern | undefined;
  rootDir?: string;
}
export declare const macroPlugin: (
  opts?: MacroPluginOptions
) => Promise<Plugin>;
export declare function parseId(originalId: string): {
  originalId: string;
  pathId: string;
  query: string;
  params: URLSearchParams;
};
