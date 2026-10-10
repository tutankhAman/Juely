// Recovered speech-to-text providers (curl templates) from Pluely 1.1.0 .rodata.
export interface SttProvider {
  id: string;
  name: string;
  curl: string;
  responseContentPath: string;
  streaming: boolean;
}

export const STT_PROVIDERS: SttProvider[] = [{id:"openai-whisper",name:"OpenAI Whisper",curl:`curl -X POST "https://api.openai.com/v1/audio/transcriptions" \\
      -H "Authorization: Bearer {{API_KEY}}" \\
      -F "file={{AUDIO}}" \\
      -F "model={{MODEL}}"`,responseContentPath:"text",streaming:!1},{id:"groq",name:"Groq Whisper",curl:`curl -X POST https://api.groq.com/openai/v1/audio/transcriptions \\
      -H "Authorization: bearer {{API_KEY}}" \\
      -F "file={{AUDIO}}" \\
      -F model={{MODEL}} \\
      -F temperature=0 \\
      -F response_format=text \\
      -F language=en`,responseContentPath:"text",streaming:!1},{id:"elevenlabs-stt",name:"ElevenLabs Speech-to-Text",curl:`curl -X POST "https://api.elevenlabs.io/v1/speech-to-text" \\
      -H "xi-api-key: {{API_KEY}}" \\
      -F "file={{AUDIO}}" \\
      -F "model_id={{MODEL}}"`,responseContentPath:"text",streaming:!1},{id:"google-stt",name:"Google Speech-to-Text",curl:`curl -X POST "https://speech.googleapis.com/v1/speech:recognize" \\
      -H "Authorization: Bearer {{API_KEY}}" \\
      -H "Content-Type: application/json" \\
      -H "x-goog-user-project: {{PROJECT_ID}}" \\
      -d '{
        "config": {
          "encoding": "LINEAR16", 
          "sampleRateHertz": 16000,
          "languageCode": "en-US"
        },
        "audio": {
          "content": "{{AUDIO}}"
        }
      }'`,responseContentPath:"results[0].alternatives[0].transcript",streaming:!1},{id:"deepgram-stt",name:"Deepgram Speech-to-Text (Dev Pro)",curl:`curl -X POST "https://api.deepgram.com/v1/listen?model={{MODEL}}&language={{LANGUAGE}}&diarize=true&smart_format=true" \\
      -H "Authorization: TOKEN {{API_KEY}}" \\
      -H "Content-Type: audio/wav" \\
      --data-binary {{AUDIO}}`,responseContentPath:"results.channels[0].alternatives[0].transcript",streaming:!1},{id:"azure-stt",name:"Azure Speech-to-Text",curl:`curl -X POST "https://{{REGION}}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1?language=en-US" \\
      -H "Ocp-Apim-Subscription-Key: {{API_KEY}}" \\
      -H "Content-Type: audio/wav" \\
      --data-binary {{AUDIO}}`,responseContentPath:"DisplayText",streaming:!1},{id:"speechmatics-stt",name:"Speechmatics",curl:`curl -X POST "https://asr.api.speechmatics.com/v2/jobs" \\
      -H "Authorization: Bearer {{API_KEY}}" \\
      -F "data_file={{AUDIO}}" \\
      -F 'config={"type": "transcription", "transcription_config": {"language": "en"}}'`,responseContentPath:"job.id",streaming:!1},{id:"rev-ai-stt",name:"Rev.ai Speech-to-Text",curl:`curl -X POST "https://api.rev.ai/speechtotext/v1/jobs" \\
      -H "Authorization: Bearer {{API_KEY}}" \\
      -F "media={{AUDIO}}" \\
      -F "options={{OPTIONS}}"`,responseContentPath:"id",streaming:!1},{id:"ibm-watson-stt",name:"IBM Watson Speech-to-Text",curl:`curl -X POST "https://api.us-south.speech-to-text.watson.cloud.ibm.com/v1/recognize" \\
      -H "Authorization: Basic {{API_KEY}}" \\
      -H "Content-Type: audio/wav" \\
      --data-binary {{AUDIO}}`,responseContentPath:"results[0].alternatives[0].transcript",streaming:!1}];
