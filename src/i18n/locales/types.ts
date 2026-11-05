/** A translation must provide exactly the keys of the Italian reference file, as strings. */
export type Translation<T> = { [K in keyof T]: string };
