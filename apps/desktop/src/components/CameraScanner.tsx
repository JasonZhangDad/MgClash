import { useEffect, useRef, useState } from "react";

/** How often a frame is handed to the decoder while the camera is open. */
const SCAN_INTERVAL_MS = 400;

interface CameraScannerProps {
  busy: boolean;
  t: (text: string) => string;
  /// Decodes one still frame; resolves with the link a QR code carries.
  decode: (image: Uint8Array) => Promise<string>;
  onLink: (link: string) => void;
  onError: (message: string) => void;
}

/**
 * Reads a sharing link off the camera.
 *
 * The frame is decoded by the same Rust command the image-file import uses, so
 * a scanned link goes through one validation path rather than two.
 */
export function CameraScanner({
  busy,
  t,
  decode,
  onLink,
  onError,
}: CameraScannerProps) {
  const video = useRef<HTMLVideoElement>(null);
  const [stream, setStream] = useState<MediaStream | null>(null);

  useEffect(() => {
    if (stream === null) {
      return undefined;
    }
    const element = video.current;
    if (element !== null) {
      element.srcObject = stream;
      try {
        // Autoplay refusal is not fatal, and a runtime without playback still
        // holds the stream open for the frame grab.
        void element.play()?.catch(() => {});
      } catch {
        // Same reasoning: playback is a preview, not the scan.
      }
    }
    let cancelled = false;
    const timer = setInterval(() => {
      const frame = captureFrame(video.current);
      if (frame === null) {
        return;
      }
      decode(frame).then(
        (link) => {
          if (!cancelled && link.trim() !== "") {
            onLink(link.trim());
          }
        },
        () => {
          // A frame without a readable code is the normal case, not an error.
        },
      );
    }, SCAN_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [decode, onLink, stream]);

  const stop = () => {
    stream?.getTracks().forEach((track) => track.stop());
    setStream(null);
  };

  return (
    <div className="camera-scan">
      {stream === null ? (
        <button
          type="button"
          aria-label={t("用摄像头扫描二维码")}
          disabled={busy}
          onClick={() => {
            void navigator.mediaDevices
              .getUserMedia({ video: { facingMode: "environment" } })
              .then(setStream, (failure: unknown) => {
                onError(
                  failure instanceof Error
                    ? failure.message
                    : t("无法打开摄像头"),
                );
              });
          }}
        >
          {t("用摄像头扫描")}
        </button>
      ) : (
        <>
          <video ref={video} aria-label={t("摄像头预览")} muted playsInline />
          <button type="button" aria-label={t("停止扫描")} onClick={stop}>
            {t("停止扫描")}
          </button>
        </>
      )}
    </div>
  );
}

/**
 * One frame as PNG bytes, or `null` while the camera has not produced a
 * picture yet — which is every frame before the stream settles.
 */
function captureFrame(video: HTMLVideoElement | null): Uint8Array | null {
  if (video === null || video.videoWidth === 0 || video.videoHeight === 0) {
    return null;
  }
  const canvas = document.createElement("canvas");
  canvas.width = video.videoWidth;
  canvas.height = video.videoHeight;
  const context = canvas.getContext("2d");
  if (context === null) {
    return null;
  }
  context.drawImage(video, 0, 0, canvas.width, canvas.height);
  const encoded = canvas.toDataURL("image/png").split(",")[1] ?? "";
  const binary = atob(encoded);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}
