#!/usr/bin/env python3
"""
Simple proxy server that pipes LLM requests to `claude -p`.
Mimics the Anthropic API format so our AnthropicClient works with it.

Usage:
    python scripts/claude_proxy.py

Then set api_base to http://localhost:5000 in your requests.
"""

import subprocess
import json
from http.server import HTTPServer, BaseHTTPRequestHandler

class ClaudeProxyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({'status': 'ok'}).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path != '/v1/messages':
            self.send_response(404)
            self.end_headers()
            return

        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length)

        try:
            data = json.loads(body)
        except json.JSONDecodeError as e:
            self.send_error(400, f'Invalid JSON: {e}')
            return

        # Extract the prompt
        messages = data.get('messages', [])
        system = data.get('system', '')

        # Build the prompt for claude CLI
        prompt_parts = []
        if system:
            prompt_parts.append(f"{system}\n")

        for msg in messages:
            content = msg.get('content', '')
            prompt_parts.append(content)

        full_prompt = '\n'.join(prompt_parts)

        print(f"\n{'='*50}")
        print(f"PROMPT:")
        print(full_prompt[:500] + "..." if len(full_prompt) > 500 else full_prompt)
        print('='*50)

        try:
            # Call claude CLI
            result = subprocess.run(
                ['claude', '-p', full_prompt],
                capture_output=True,
                text=True,
                timeout=120
            )

            response_text = result.stdout.strip()

            if result.returncode != 0:
                self.send_response(500)
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(json.dumps({
                    'error': {'message': f"Claude CLI error: {result.stderr}"}
                }).encode())
                return

            print(f"\nRESPONSE:")
            print(response_text[:500] + "..." if len(response_text) > 500 else response_text)
            print('='*50 + '\n')

            # Return in Anthropic API format
            response = {
                'content': [{'type': 'text', 'text': response_text}],
                'model': 'claude-cli',
                'usage': {
                    'input_tokens': len(full_prompt.split()),
                    'output_tokens': len(response_text.split())
                }
            }

            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())

        except subprocess.TimeoutExpired:
            self.send_response(504)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({
                'error': {'message': 'Claude CLI timeout'}
            }).encode())
        except Exception as e:
            self.send_response(500)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({
                'error': {'message': str(e)}
            }).encode())

    def log_message(self, format, *args):
        # Quieter logging
        pass

if __name__ == '__main__':
    port = 5000
    server = HTTPServer(('localhost', port), ClaudeProxyHandler)
    print(f"Claude proxy running on http://localhost:{port}")
    print(f"Use api_base='http://localhost:{port}' in your requests")
    print("Press Ctrl+C to stop\n")
    server.serve_forever()
