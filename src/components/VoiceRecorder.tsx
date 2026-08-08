import { useEffect, useRef, useState } from 'react';
import { Mic, Square } from 'lucide-react';
import { useVoiceInput } from '../hooks/useVoiceInput';

interface Props {
  onTranscript: (text: string) => void;
}

// 简洁录音按钮：录音期间把识别片段攒在缓冲区，结束后一次性回调，避免反复触发上层提取。
export default function VoiceRecorder({ onTranscript }: Props) {
  const bufferRef = useRef<string[]>([]);
  const voice = useVoiceInput((text) => {
    bufferRef.current.push(text);
  });

  // 录音超时检测：录音 5 秒后仍无临时文本，提示用户
  const [noAudioWarning, setNoAudioWarning] = useState(false);
  const recordingStartRef = useRef<number>(0);

  useEffect(() => {
    if (voice.recording) {
      recordingStartRef.current = Date.now();
      setNoAudioWarning(false);
      const timer = setTimeout(() => {
        // 5秒后检查是否有临时文本或已经有结果
        if (!bufferRef.current.length) {
          setNoAudioWarning(true);
        }
      }, 5000);
      return () => clearTimeout(timer);
    } else {
      setNoAudioWarning(false);
    }
  }, [voice.recording]);

  // 当 interimText 变化时清除超时警告
  useEffect(() => {
    if (voice.interimText) setNoAudioWarning(false);
  }, [voice.interimText]);

  useEffect(() => {
    if (!voice.recording && !voice.transcribing && bufferRef.current.length > 0) {
      const text = bufferRef.current.join(' ');
      bufferRef.current = [];
      onTranscript(text);
    }
  }, [voice.recording, voice.transcribing]);

  return (
    <div className="rounded-lg border bg-secondary p-3">
      <div className="flex items-center gap-2">
        <button
          type="button"
          className={`rounded-lg px-4 py-2 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50 inline-flex items-center gap-1.5 ${
            voice.recording
              ? 'animate-pulse bg-danger-light text-danger'
              : 'border border-line bg-card text-text-primary hover:bg-surface'
          }`}
          disabled={!voice.supported || voice.transcribing}
          onClick={() => voice.toggle()}
        >
          {voice.recording ? (<><Square size={14} aria-hidden="true" /> 停止</>) : (<><Mic size={14} aria-hidden="true" /> 语音转文字</>)}
        </button>
        {voice.transcribing && <span className="text-sm text-text-secondary">录音上传转写中...</span>}
        {voice.recording && (
          <span className="text-sm text-text-secondary">
            正在聆听{voice.interimText ? `：${voice.interimText}` : '...'}
          </span>
        )}
      </div>
      {!voice.supported && (
        <p className="mt-2 text-xs text-text-secondary">
          {voice.unsupportedReason || '当前浏览器不支持语音输入，请使用文字输入。'}
        </p>
      )}
      {noAudioWarning && !voice.error && (
        <p className="mt-2 text-sm text-warning">未检测到音频输入，请确认麦克风已连接并允许访问</p>
      )}
      {voice.error && <p className="mt-2 text-sm text-danger">{voice.error}</p>}
    </div>
  );
}
