// 语音输入降级链：Web Speech API（端侧识别，首选）→ MediaRecorder 录音上传
// 服务端 /api/voice/transcribe → 两者都不可用时给出友好提示。
import { useCallback, useEffect, useRef, useState } from 'react';
import { transcribeAudio } from '../services/whisper';

// Web Speech API 尚无内置 TS 类型，这里声明用到的最小子集。
interface SpeechRecognitionAlternativeLike {
  transcript: string;
}

interface SpeechRecognitionResultLike {
  isFinal: boolean;
  0: SpeechRecognitionAlternativeLike;
}

interface SpeechRecognitionEventLike {
  resultIndex: number;
  results: {
    length: number;
    [index: number]: SpeechRecognitionResultLike;
  };
}

interface SpeechRecognitionLike {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
}

type SpeechRecognitionCtor = new () => SpeechRecognitionLike;

const UNSUPPORTED_MESSAGE = '当前浏览器/服务端不支持语音转写，请使用文字输入';

function getSpeechRecognitionCtor(): SpeechRecognitionCtor | null {
  const w = window as unknown as Record<string, unknown>;
  return (w.SpeechRecognition as SpeechRecognitionCtor | undefined)
    ?? (w.webkitSpeechRecognition as SpeechRecognitionCtor | undefined)
    ?? null;
}

function canUseMediaRecorder(): boolean {
  return typeof MediaRecorder !== 'undefined' && !!navigator.mediaDevices?.getUserMedia;
}

export interface VoiceInput {
  /** 任一语音方案可用（不代表服务端已配置转写） */
  supported: boolean;
  /** 不支持时的具体原因，供 UI 显示 */
  unsupportedReason: string;
  /** 正在录音/识别 */
  recording: boolean;
  /** 录音结束后正在上传转写（仅 MediaRecorder 降级路径） */
  transcribing: boolean;
  /** Web Speech 的临时识别文本，可用于界面预览 */
  interimText: string;
  error: string;
  start: () => Promise<void>;
  stop: () => void;
  toggle: () => Promise<void>;
}

export function useVoiceInput(onTranscript: (text: string) => void): VoiceInput {
  const [recording, setRecording] = useState(false);
  const [transcribing, setTranscribing] = useState(false);
  const [interimText, setInterimText] = useState('');
  const [error, setError] = useState('');

  const recognitionRef = useRef<SpeechRecognitionLike | null>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;

  const hasSpeechApi = getSpeechRecognitionCtor() !== null;
  const hasMediaRecorder = canUseMediaRecorder();
  const supported = hasSpeechApi || hasMediaRecorder;

  // 计算不支持时的具体原因
  let unsupportedReason = '';
  if (!supported) {
    unsupportedReason = '当前环境不支持语音输入（Web Speech API 不可用，且无法使用 MediaRecorder 降级录音）';
  } else if (!hasSpeechApi && hasMediaRecorder) {
    // MediaRecorder 可用但依赖服务端 Whisper 转写
    unsupportedReason = '';
  }

  // 初始化诊断日志
  useEffect(() => {
    console.info('[voice-input] 诊断:', {
      webSpeechAPI: hasSpeechApi,
      mediaRecorder: hasMediaRecorder,
      supported,
    });
  }, []);

  const cleanupStream = () => {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
  };

  // 组件卸载时释放麦克风与识别器
  useEffect(() => () => {
    recognitionRef.current?.stop();
    if (mediaRecorderRef.current?.state === 'recording') {
      mediaRecorderRef.current.stop();
    }
    cleanupStream();
  }, []);

  const startSpeechRecognition = (Ctor: SpeechRecognitionCtor) => {
    const recognition = new Ctor();
    recognition.lang = 'zh-CN';
    recognition.continuous = true;
    recognition.interimResults = true;

    recognition.onresult = (event) => {
      let interim = '';
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const result = event.results[i];
        const transcript = result[0]?.transcript ?? '';
        if (result.isFinal) {
          if (transcript.trim()) {
            onTranscriptRef.current(transcript.trim());
          }
        } else {
          interim += transcript;
        }
      }
      setInterimText(interim);
    };

    recognition.onerror = (event) => {
      if (event.error === 'not-allowed' || event.error === 'service-not-allowed') {
        setError('未获得麦克风权限，请在浏览器中允许访问麦克风，或使用文字输入');
      } else if (event.error === 'no-speech') {
        setError('未检测到语音，请重试或使用文字输入');
      } else if (event.error !== 'aborted') {
        setError(UNSUPPORTED_MESSAGE);
      }
    };

    recognition.onend = () => {
      recognitionRef.current = null;
      setRecording(false);
      setInterimText('');
    };

    recognitionRef.current = recognition;
    recognition.start();
    setRecording(true);
  };

  const startMediaRecorder = async () => {
    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      setError('麦克风权限被拒绝，请在浏览器设置中允许麦克风访问');
      setRecording(false);
      return;
    }

    const mimeType = MediaRecorder.isTypeSupported('audio/webm') ? 'audio/webm' : undefined;
    const recorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);
    chunksRef.current = [];

    recorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        chunksRef.current.push(event.data);
      }
    };

    recorder.onstop = async () => {
      cleanupStream();
      mediaRecorderRef.current = null;
      setRecording(false);

      const blob = new Blob(chunksRef.current, { type: recorder.mimeType || 'audio/webm' });
      chunksRef.current = [];
      if (blob.size === 0) {
        setError('未录到声音，请重试');
        return;
      }
      setTranscribing(true);
      try {
        const text = await transcribeAudio(blob);
        if (text.trim()) {
          onTranscriptRef.current(text.trim());
        } else {
          setError('未识别到语音内容，请重试或使用文字输入');
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : UNSUPPORTED_MESSAGE;
        // 如果是 501 则给出更具体的配置提示
        if (msg.includes('未配置')) {
          setError('语音转写服务未配置（需要安装 Whisper 并设置 RG_WHISPER_CMD 环境变量）');
        } else {
          setError(msg);
        }
      } finally {
        setTranscribing(false);
      }
    };

    streamRef.current = stream;
    mediaRecorderRef.current = recorder;
    recorder.start();
    setRecording(true);
  };

  const start = useCallback(async () => {
    if (recording || transcribing) return;
    setError('');
    setInterimText('');

    const Ctor = getSpeechRecognitionCtor();
    if (Ctor) {
      startSpeechRecognition(Ctor);
      return;
    }
    if (canUseMediaRecorder()) {
      await startMediaRecorder();
      return;
    }
    setError(UNSUPPORTED_MESSAGE);
  }, [recording, transcribing]);

  const stop = useCallback(() => {
    recognitionRef.current?.stop();
    if (mediaRecorderRef.current?.state === 'recording') {
      mediaRecorderRef.current.stop();
    }
  }, []);

  const toggle = useCallback(async () => {
    if (recording) {
      stop();
    } else {
      await start();
    }
  }, [recording, start, stop]);

  return { supported, unsupportedReason, recording, transcribing, interimText, error, start, stop, toggle };
}
