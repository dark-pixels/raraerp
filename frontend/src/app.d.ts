// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		interface Locals {
			user: import('$types/auth').AuthUser | null;
		}
		interface PageData {
			user?: import('$types/auth').AuthUser | null;
		}
		interface Error {
			message: string;
			code?: string;
		}
	}
}

export {};
