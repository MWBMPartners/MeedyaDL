import json

def update_config(library_paths, config_path="config.json"):
    """
    Updates the paths to third-party libraries in GAMDL's config.json.
    """
    try:
        with open(config_path, "r") as file:
            config = json.load(file)

        config.update(library_paths)

        with open(config_path, "w") as file:
            json.dump(config, file, indent=4)

        print("config.json updated successfully.")
    except Exception as e:
        print(f"Failed to update config.json: {e}")