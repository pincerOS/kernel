import socket

# TCP configuration
host = '127.0.0.1'  # Localhost IP
port = 2222         # Port to send the packet to

# Message to send
message = b'cowscowscows'

# Create a TCP socket
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

# Connect to the server
sock.connect((host, port))

# Send the message to the server
sock.sendall(message)

# Close the socket
sock.close()

print(f"Message sent to {host}:{port}")