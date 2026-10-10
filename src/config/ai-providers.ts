// Recovered LLM chat providers (curl templates) from Pluely 1.1.0 .rodata.
export interface AiProvider {
  id: string;
  curl: string;
  responseContentPath: string;
  streaming: boolean;
}

export const AI_PROVIDERS: AiProvider[] = [{id:"openai",curl:`curl https://api.openai.com/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer {{API_KEY}}" \\
  -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"claude",curl:`curl https://api.anthropic.com/v1/messages \\
  -H "x-api-key: {{API_KEY}}" \\
  -H "anthropic-version: 2023-06-01" \\
  -H "anthropic-dangerous-direct-browser-access: true" \\
  -H "content-type: application/json" \\
  -d '{
    "model": "{{MODEL}}",
    "system": "{{SYSTEM_PROMPT}}",
    "messages": [{"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "{{IMAGE}}"}}]}],
    "max_tokens": 1024
  }'`,responseContentPath:"content[0].text",streaming:!0},{id:"grok",curl:`curl https://api.x.ai/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer {{API_KEY}}" \\
  -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"gemini",curl:`curl "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions" \\
  -H "Authorization: Bearer {{API_KEY}}" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"mistral",curl:`curl https://api.mistral.ai/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer {{API_KEY}}" \\
  -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": "data:image/png;base64,{{IMAGE}}"}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"cohere",curl:`curl -X POST https://api.cohere.ai/v2/chat \\
    -H "Authorization: Bearer {{API_KEY}}" \\
    -H "Content-Type: application/json" \\
    -d '{
      "model": "{{MODEL}}",
      "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
    }'`,responseContentPath:"message.content[0].text",streaming:!0},{id:"groq",curl:`curl https://api.groq.com/openai/v1/chat/completions     -H "Content-Type: application/json"     -H "Authorization: Bearer {{API_KEY}}"     -d '{
      "model": "{{MODEL}}",
      "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}],
      "temperature": 1,
      "max_completion_tokens": 8192,
      "top_p": 1,
      "stream": true
    }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"perplexity",curl:`curl -X POST https://api.perplexity.ai/chat/completions \\
  -H "Authorization: Bearer {{API_KEY}}" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"openrouter",curl:`  curl https://openrouter.ai/api/v1/chat/completions   -H "Content-Type: application/json"   -H "Authorization: Bearer {{API_KEY}}"   -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0},{id:"ollama",curl:`curl -X POST http://localhost:11434/v1/chat/completions \\
    -H "Authorization: Bearer {{API_KEY}}" \\
    -H "Content-Type: application/json" \\
    -d '{
    "model": "{{MODEL}}",
    "messages": [{"role": "system", "content": "{{SYSTEM_PROMPT}}"}, {"role": "user", "content": [{"type": "text", "text": "{{TEXT}}"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,{{IMAGE}}"}}]}]
  }'`,responseContentPath:"choices[0].message.content",streaming:!0}];
