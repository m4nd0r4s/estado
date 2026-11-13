import path from 'node:path';
import { repositoryUrl } from '../site.ts';

/** The repository root; Vite runs from `site/`. */
export const repoRoot = path.resolve(process.cwd(), '..');

/** The base path the site is served under (`/estado` on GitHub Pages). */
export const basePath = process.env.BASE_PATH ?? '';

/** A link to a file or folder of the repository on GitHub. */
export function githubUrl(repoPath: string): string {
	return `${repositoryUrl}/blob/main/${repoPath}`;
}

/** Reads a path relative to the repository root as POSIX. */
export function toRepoPath(absolutePath: string): string {
	return path.relative(repoRoot, absolutePath).split(path.sep).join('/');
}
