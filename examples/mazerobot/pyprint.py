import json
import sys
import os

def c_to_json(file_path):
    # Check if file exists
    if not os.path.exists(file_path):
        print(f"Error: File '{file_path}' not found.")
        return

    code_dict = {}

    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            # enumerate(f, 1) starts the line count at 1
            for line_num, line_content in enumerate(f, 1):
                # .rstrip('\n') removes the trailing newline character
                code_dict[str(line_num)] = line_content.rstrip('\n')

        # Output the dictionary as formatted JSON
        print(json.dumps(code_dict, indent=4))

    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    # Ensure a filename was provided as an argument
    if len(sys.argv) > 1:
        c_to_json(sys.argv[1])
    else:
        print("Usage: python3 c_to_json.py <filename.c>")