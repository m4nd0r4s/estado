import { navigation } from '#lib/server/docs.ts';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = () => ({ navigation: navigation() });
