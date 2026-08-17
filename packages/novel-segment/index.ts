export { POSTAG } from '@novel-segment/postag/lib/postag/ids';
export { stringify } from '@novel-segment/stringify';
import { Segment as NativeSegment } from 'novel-segment-native';

export const Segment = NativeSegment;
export default NativeSegment;
export const useDefault = <T>(segment: T): T => segment;
export { create, createSegment } from 'novel-segment-native';
