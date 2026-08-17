export interface INativeSegmentOptions {
  autoCjk?: boolean;
  allMod?: boolean;
  all_mod?: boolean;
  nodeNovelMode?: boolean;
  node_novel_mode?: boolean;
  convertSynonym?: boolean;
  optionsDoSegment?: IOptionsDoSegment;
}

export interface IOptionsDoSegment {
  convertSynonym?: boolean;
  simple?: boolean;
  stripPunctuation?: boolean;
  stripStopword?: boolean;
  stripSpace?: boolean;
}

export interface IWord {
  w: string;
  p?: number;
  f?: number;
}

export class Segment {
  constructor(options?: INativeSegmentOptions);
  static withNodeNovelDefault(): Segment;
  static stringify(words: Array<IWord | string> | string | null | undefined): string;
  doSegment(text: string, options?: IOptionsDoSegment): IWord[];
  stringify(wordsOrText: Array<IWord | string> | string, options?: IOptionsDoSegment): string;
  addWord(spec: string, p?: number, f?: number): this;
  addSynonym(canonical: string, variants: string[]): this;
  addBlacklist(word: string): this;
  inited: boolean;
  options: INativeSegmentOptions;
}

export function stringify(words: Array<IWord | string> | string | null | undefined): string;
export function create(options?: INativeSegmentOptions): Segment;
export function createSegment(options?: INativeSegmentOptions): Segment;
