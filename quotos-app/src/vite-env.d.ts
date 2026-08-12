/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Displayed app name, injected at packaging time so v2 can run side by
   * side with v1 without renaming the product in source. Defaults to
   * "Quotos". See quotos-app/README.md "Packaging a side-by-side build". */
  readonly VITE_APP_LABEL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
