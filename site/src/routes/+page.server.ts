import { landingData } from '#lib/server/landing.ts';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = () => landingData();
