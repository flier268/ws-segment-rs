import { Segment as NativeSegment, IWord, IOptionsDoSegment } from 'novel-segment-native';
export { POSTAG } from '@novel-segment/postag/lib/postag/ids';
export { stringify } from '@novel-segment/stringify';

export const Segment: typeof NativeSegment;
export default NativeSegment;
export function useDefault<T>(segment: T): T;
export function create(options?: ConstructorParameters<typeof NativeSegment>[0]): NativeSegment;
export function createSegment(options?: ConstructorParameters<typeof NativeSegment>[0]): NativeSegment;
export type { IWord, IOptionsDoSegment };
