// swagger-ui-dist ships no types; the API reference page (/docs/api) uses only this.
declare module 'swagger-ui-dist/swagger-ui-bundle.js' {
	interface SwaggerUIOptions {
		url: string;
		domNode: HTMLElement;
		deepLinking?: boolean;
		docExpansion?: 'list' | 'full' | 'none';
		defaultModelsExpandDepth?: number;
		withCredentials?: boolean;
		persistAuthorization?: boolean;
		tryItOutEnabled?: boolean;
	}
	const SwaggerUIBundle: (options: SwaggerUIOptions) => unknown;
	export default SwaggerUIBundle;
}
