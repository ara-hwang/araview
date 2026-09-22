// WebCodecs ImageDecoder (Chromium 94+, WebView2 해당).
// TypeScript 5.6 DOM lib에는 아직 없어 필요한 최소 표면만 선언한다.
// 미지원 환경은 런타임 feature detect(`supportsAnimationControl`)로 거른다.

type ImageDecoderInit = {
  data: BufferSource | ReadableStream<Uint8Array>
  type: string
  colorSpaceConversion?: "default" | "none"
  desiredWidth?: number
  desiredHeight?: number
  preferAnimation?: boolean
}

type ImageDecodeOptions = {
  frameIndex?: number
  completeFramesOnly?: boolean
}

type ImageDecodeResult = {
  image: VideoFrame
  complete: boolean
}

interface ImageTrack {
  readonly animated: boolean
  readonly frameCount: number
  /** 무한 반복이면 Infinity. */
  readonly repetitionCount: number
  readonly selected: boolean
}

interface ImageTrackList {
  readonly ready: Promise<void>
  readonly length: number
  readonly selectedIndex: number
  readonly selectedTrack: ImageTrack | null
}

interface ImageDecoder {
  readonly tracks: ImageTrackList
  readonly completed: Promise<void>
  readonly type: string
  readonly complete: boolean
  decode(options?: ImageDecodeOptions): Promise<ImageDecodeResult>
  close(): void
  reset(): void
}

declare var ImageDecoder: {
  prototype: ImageDecoder
  new (init: ImageDecoderInit): ImageDecoder
}
