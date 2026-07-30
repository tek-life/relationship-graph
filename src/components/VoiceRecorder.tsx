import { useState } from 'react';
import { transcribeAudio } from '../services/whisper';

interface Props {
  onTranscript: (text: string) => void;
}

export default function VoiceRecorder({ onTranscript }: Props) {
  const [audioPath, setAudioPath] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleTranscribe = async () => {
    if (!audioPath.trim()) return;
    setLoading(true);
    setError('');
    try {
      const text = await transcribeAudio(audioPath.trim());
      onTranscript(text);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="rounded-lg border bg-slate-50 p-3">
      <label className="text-sm font-medium text-slate-700">语音转文字</label>
      <div className="mt-2 flex gap-2">
        <input
          className="input"
          placeholder="本地音频文件路径，例如 C:\\temp\\meeting.wav"
          value={audioPath}
          onChange={(event) => setAudioPath(event.target.value)}
        />
        <button type="button" className="btn-secondary whitespace-nowrap" onClick={handleTranscribe} disabled={loading}>
          {loading ? '转写中...' : '转写'}
        </button>
      </div>
      {error && <p className="mt-2 text-sm text-red-600">{error}</p>}
      <p className="mt-2 text-xs text-slate-500">需要本机安装 whisper-cli，并把模型放到应用数据目录的 models/ggml-base.bin。</p>
    </div>
  );
}
