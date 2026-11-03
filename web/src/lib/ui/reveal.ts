import type { Action } from 'svelte/action';

let observer: IntersectionObserver | null = null;

/**
 * Fades an element up when it first scrolls into view (`data-reveal` in app.css).
 * `delay` staggers siblings. One observer serves the whole page.
 */
export const reveal: Action<HTMLElement, number | undefined> = (node, delay = 0) => {
	node.dataset.reveal = '';
	node.style.setProperty('--reveal-delay', `${delay}ms`);
	if (typeof IntersectionObserver === 'undefined') {
		node.classList.add('revealed');
		return;
	}
	observer ??= new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				entry.target.classList.add('revealed');
				observer?.unobserve(entry.target);
			}
		},
		{ rootMargin: '0px 0px -8% 0px', threshold: 0.08 }
	);
	observer.observe(node);
	return {
		destroy: () => observer?.unobserve(node)
	};
};
