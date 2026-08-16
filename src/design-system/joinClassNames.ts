/** Joins CSS Modules class names, dropping every falsy value, so a
 * component can compose a base class with conditional variant classes in
 * one expression instead of building an array by hand at each call site. */
export function joinClassNames(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(" ");
}
