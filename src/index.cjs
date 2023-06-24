exports.macroPlugin = async (opts) => {
  const { macroPlugin } = await import("./index.js");
  return macroPlugin(opts);
};
