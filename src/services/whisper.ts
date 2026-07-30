import { invoke } from '@tauri-apps/api/core';

export async function transcribeAudio(audioPath: string): Promise<string> {
  const result = await invoke<{ text: string }>('transcribe_audio', { audioPath });
  return result.text;
}
