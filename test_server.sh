#!/bin/bash
echo "Testing if web server is running..."
curl -s http://localhost:8080/api/status || echo "Server not responding"
echo ""
echo "Checking port 8080..."
lsof -ti:8080 && echo "Port 8080 is in use" || echo "Port 8080 is free"


