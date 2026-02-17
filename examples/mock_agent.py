import sys
import json
import logging

# Configure logging to stderr (since stdout is for MCP protocol)
logging.basicConfig(stream=sys.stderr, level=logging.INFO)

def main():
    logging.info("Mock Agent Started")
    
    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break
            
            request = json.loads(line)
            req_id = request.get("id")
            method = request.get("method")
            
            logging.info(f"Received request: {method}")
            
            response = None
            
            if method == "initialize":
                response = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "mock-agent",
                            "version": "1.0.0"
                        }
                    }
                }
            elif method == "notifications/initialized":
                continue
            elif method == "tools/list":
                response = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "tools": [
                            {
                                "name": "calculator",
                                "description": "A basic calculator tool",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "a": {"type": "number"},
                                        "b": {"type": "number"},
                                        "op": {"type": "string"}
                                    }
                                }
                            }
                        ]
                    }
                }
            elif method == "tools/call":
                response = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": "42"
                            }
                        ]
                    }
                }
            elif method == "ping":
                 response = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {}
                }
            else:
                if req_id is not None:
                    response = {
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": {
                            "code": -32601,
                            "message": "Method not found"
                        }
                    }

            if response:
                sys.stdout.write(json.dumps(response) + "\n")
                sys.stdout.flush()
                
        except Exception as e:
            logging.error(f"Error: {e}")
            break

if __name__ == "__main__":
    main()
