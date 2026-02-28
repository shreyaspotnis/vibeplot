/* tslint:disable */
/* eslint-disable */

export function get_rotation(): Float32Array;

export function get_zoom(): number;

export function load_cube_model(): void;

export function load_model(model_text: string): void;

export function load_pyramid_model(): void;

export function reset_rotation(): void;

export function reset_zoom(): void;

export function run(): Promise<void>;

export function set_rotation(x: number, y: number): void;

export function set_zoom(scale: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly get_rotation: () => any;
    readonly load_cube_model: () => [number, number];
    readonly load_model: (a: number, b: number) => [number, number];
    readonly load_pyramid_model: () => [number, number];
    readonly reset_rotation: () => void;
    readonly reset_zoom: () => void;
    readonly run: () => void;
    readonly set_rotation: (a: number, b: number) => void;
    readonly set_zoom: (a: number) => void;
    readonly get_zoom: () => number;
    readonly wasm_bindgen__closure__destroy__h12d7c8bae7649a81: (a: number, b: number) => void;
    readonly wasm_bindgen__closure__destroy__h094ed43c40b4c4f5: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h0a82ef6223862297: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hdc77070b25da1a99: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h33a426a7a5775c3c: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
