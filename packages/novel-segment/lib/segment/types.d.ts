export interface IOptionsDoSegment {
  simple?: boolean;
  stripPunctuation?: boolean;
  convertSynonym?: boolean;
  stripStopword?: boolean;
  stripSpace?: boolean;
}

export interface IOptionsSegment {
  autoCjk?: boolean;
  all_mod?: boolean;
  nodeNovelMode?: boolean;
  optionsDoSegment?: IOptionsDoSegment;
}

export interface IWord {
  w: string;
  p?: number;
  f?: number;
}
