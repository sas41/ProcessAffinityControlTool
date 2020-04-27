using System;
using System.Linq;
using System.Collections.Generic;
using Newtonsoft.Json;
using System.IO;
using System.Diagnostics;

namespace ProcessAffinityControlTool
{
    class Program
    {
        const string version = "1.1.0";

        const int minArgumentCount_AddException = 4;
        const int exactArgumentCount_AddRemoveProcess = 2;
        const int minArgumentCount_SetCores = 2;
        const int exactArgumentCount_SetPriority = 2;
        const int exactArgumentCount_SetScanInterval = 2;
        const int exactArgumentCount_SetAggressiveScanInterval = 2;
        const int exactArgumentCount_SetForceAggressiveScanInterval = 2;

        static ProcessOverwatch pow;
        static List<string> games;
        static PACTConfig conf;
        static PACTConfig pausedConf;
        static bool running;
        static int highestCoreNumber;

        static void Main(string[] args)
        {
            pow = new ProcessOverwatch();
            conf = ReadConfig();
            games = ReadGames();
            pausedConf = new PACTConfig();
            running = true;
            highestCoreNumber = Environment.ProcessorCount;

            pow.Config = conf;
            pow.Games = games;
            pow.SetTimer();
            pow.RunScan(true);

            Console.WriteLine();
            Console.WriteLine($"P.A.C.T. v{version}, by Berk (SAS41) Alyamach.");
            Console.WriteLine("Type [help] or [?] for a valid list of commands.");
            Console.WriteLine("Official Repository: https://github.com/sas41/ProcessAffinityControlTool/");
            while (true)
            {
                Console.WriteLine();
                Console.WriteLine("Enter your command:");
                string command = Console.ReadLine().ToLower();
                List<string> arguments = command.Split().ToList();

                try
                {
                    if (arguments[0] == "add" || arguments[0] == "update")
                    {
                        AddException(arguments);
                    }
                    else if (arguments[0] == "remove" || arguments[0] == "rem")
                    {
                        RemoveException(arguments);
                    }
                    else if (arguments[0] == "add_game" || arguments[0] == "ag")
                    {
                        AddGame(arguments);
                    }
                    else if (arguments[0] == "remove_game" || arguments[0] == "rg")
                    {
                        RemoveGame(arguments);
                    }
                    else if (arguments[0] == "game_cores" || arguments[0] == "gc")
                    {
                        SetGameCores(arguments);
                    }
                    else if (arguments[0] == "game_priority" || arguments[0] == "gp")
                    {
                        SetGamePriority(arguments);
                    }
                    else if (arguments[0] == "default_cores" || arguments[0] == "dc")
                    {
                        SetDefaultCores(arguments);
                    }
                    else if (arguments[0] == "default_priority" || arguments[0] == "dp")
                    {
                        SetDefaultPriority(arguments);
                    }
                    else if (arguments[0] == "scan_interval" || arguments[0] == "si")
                    {
                        SetScanInterval(arguments);
                    }
                    else if (arguments[0] == "aggressive_scan_interval" || arguments[0] == "asi")
                    {
                        SetAggressiveScanInterval(arguments);
                    }
                    else if (arguments[0] == "force_aggressive_scan" || arguments[0] == "fas")
                    {
                        SetForceAggressiveScanInterval(arguments);
                    }
                    else if (arguments[0] == "show_defaults" || arguments[0] == "sd")
                    {
                        ShowDefaults();
                    }
                    else if (arguments[0] == "show_exceptions" || arguments[0] == "se")
                    {
                        ShowExceptions();
                    }
                    else if (arguments[0] == "toggle")
                    {
                        TogglePACT();
                    }
                    else if (arguments[0] == "apply")
                    {
                        ApplyCustomConfig();
                        Console.WriteLine("Config Applied! Don't forget to save it!");
                    }
                    else if (arguments[0] == "save")
                    {
                        SaveConfig(conf);
                        Console.WriteLine("Config Saved!");
                    }
                    else if (arguments[0] == "help" || arguments[0] == "?")
                    {
                        ShowHelp();
                    }
                    else if (arguments[0] == "exit" || arguments[0] == "quit" || arguments[0] == "stop")
                    {
                        Console.WriteLine("Apply Default Config...");
                        ApplyDefaultConfig();
                        Console.WriteLine("PACT exiting...");
                        break;
                    }
                    else
                    {
                        throw new ArgumentException();
                    }
                }
                catch (ArgumentException ae)
                {
                    InvalidInputMessage();
                }
                catch (Exception ex)
                {
                    throw ex;
                }
            }
        }

        //////////////////////////////////////
        static PACTConfig ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "config.json";
            Console.WriteLine();
            Console.WriteLine($"Looking for config at: [{configPath}]...");
            if (File.Exists(configPath))
            {
                string json = File.ReadAllText(configPath);
                Console.WriteLine("Config found!");
                PACTConfig tconf = JsonConvert.DeserializeObject<PACTConfig>(json);
                return tconf;
            }
            else
            {
                Console.WriteLine("Config not found, using initial defaults...");
            }

            return new PACTConfig();
        }
        //////////////////////////////////////
        static List<string> ReadGames()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "games.txt";
            Console.WriteLine();
            Console.WriteLine($"Looking for games at: [{configPath}]...");
            if (File.Exists(configPath))
            {
                Console.WriteLine("Games found!");
                List<string> gamesList = new List<string>(File.ReadAllLines(configPath));
                gamesList = gamesList.Distinct().ToList();
                gamesList.Sort();
                return gamesList;
            }
            else
            {
                Console.WriteLine("Games not found, games.txt is missing...");
            }

            return new List<string>();
        }
        //////////////////////////////////////
        static void SaveConfig(PACTConfig conf)
        {
            string json = JsonConvert.SerializeObject(conf, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);

            string gamesPath = path + "games.txt";
            games = games.Distinct().ToList();
            games.Sort();
            File.WriteAllLines(gamesPath, games.ToArray());
        }
        //////////////////////////////////////
        static void AddException(List<string> arguments)
        {
            if (arguments.Count >= minArgumentCount_AddException)
            {
                int priority;
                List<int> cores = new List<int>();

                arguments = arguments.Skip(1).ToList();
                string exeName = arguments[0];

                if (exeName[0] == '\"')
                {
                    arguments = arguments.Skip(1).ToList();
                    string current = arguments[0];

                    while (!current.Contains('\"'))
                    {
                        exeName = $"{exeName} {current}";
                        arguments = arguments.Skip(1).ToList();
                        current = arguments[0];
                    }

                    exeName = $"{exeName} {current}";
                }

                arguments = arguments.Skip(1).ToList();

                if (int.TryParse(arguments[0], out priority))
                {
                    foreach (var str in arguments.Skip(1))
                    {
                        int number;
                        if (int.TryParse(str, out number) && number <= highestCoreNumber && number >= 0)
                        {
                            cores.Add(number);
                        }
                        else
                        {
                            throw new ArgumentException();
                        }
                    }

                    exeName = exeName.Replace("\"", "");
                    cores.Sort();

                    if (exeName.Length > 4 && exeName.Substring(exeName.Length - 4, 4) == ".exe")
                    {
                        exeName = exeName.Substring(0, exeName.Length - 4);
                    }

                    if (conf.ProcessConfigs.ContainsKey(exeName))
                    {
                        conf.ProcessConfigs[exeName] = new ProcessConfig(cores, priority);
                        Console.WriteLine($"Process [{exeName}] has been updated!");
                        Console.WriteLine($"Don't Forget to save!");
                    }
                    else
                    {
                        conf.ProcessConfigs.Add(exeName, new ProcessConfig(cores, priority));
                        Console.WriteLine($"Process [{exeName}] has been added!");
                        Console.WriteLine($"Don't Forget to save!");
                    }
                }
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void RemoveException(List<string> arguments)
        {
            if (arguments.Count >= exactArgumentCount_AddRemoveProcess && arguments[1].Replace(".exe", "").Length > 0)
            {
                string processName = string.Join(" ", arguments.Skip(1));

                if (processName.Substring(processName.Length - 4, 4) == ".exe")
                {
                    processName = processName.Substring(0, processName.Length - 4);
                }

                if (processName.Length > 0)
                {
                    if (conf.ProcessConfigs.ContainsKey(processName))
                    {
                        conf.ProcessConfigs.Remove(processName);
                        Console.WriteLine($"Process [{processName}] has been removed!");
                        Console.WriteLine($"Don't Forget to save!");
                    }
                    else
                    {
                        Console.WriteLine($"Process [{processName}] is not configured!");
                    }
                }
                else
                {
                    throw new ArgumentException();
                }
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void AddGame(List<string> arguments)
        {
            if (arguments.Count >= exactArgumentCount_AddRemoveProcess)
            {
                string processName = string.Join(" ", arguments.Skip(1));

                if (processName.Substring(processName.Length - 4, 4) == ".exe")
                {
                    processName = processName.Substring(0, processName.Length - 4);
                }

                if (processName.Length > 0)
                {
                    if (!games.Contains(processName))
                    {
                        games.Add(processName);
                        Console.WriteLine("New game added!");
                        Console.WriteLine($"Don't Forget to save!");
                    }
                    else
                    {
                        Console.WriteLine("Game already included!");
                    }
                }
                else
                {
                    throw new ArgumentException();
                }
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void RemoveGame(List<string> arguments)
        {
            if (arguments.Count >= exactArgumentCount_AddRemoveProcess)
            {
                string processName = string.Join(" ", arguments.Skip(1));

                if (processName.Substring(processName.Length - 4, 4) == ".exe")
                {
                    processName = processName.Substring(0, processName.Length - 4);
                }

                if (processName.Length > 0)
                {
                    if (games.Contains(processName))
                    {
                        games.Remove(processName);
                        Console.WriteLine("Game removed!");
                        Console.WriteLine($"Don't Forget to save!");
                    }
                    else
                    {
                        Console.WriteLine("Game not found!");
                    }
                }
                else
                {
                    throw new ArgumentException();
                }
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetGameCores(List<string> arguments)
        {
            if (arguments.Count >= minArgumentCount_SetCores)
            {
                List<int> gameCores = new List<int>();

                foreach (var str in arguments.Skip(1))
                {
                    int number;
                    if (int.TryParse(str, out number) && number <= highestCoreNumber && number >= 0)
                    {
                        gameCores.Add(number);
                    }
                    else
                    {
                        throw new ArgumentException();
                    }
                }
                gameCores.Sort();
                conf.GameConfig = new ProcessConfig(gameCores, conf.GameConfig.PriorityNumber);
                Console.WriteLine("Game Cores Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetGamePriority(List<string> arguments)
        {
            int gamePriority;
            if (arguments.Count == exactArgumentCount_SetPriority && int.TryParse(arguments[1], out gamePriority))
            {
                conf.GameConfig = new ProcessConfig(conf.GameConfig.CoreList, gamePriority);
                Console.WriteLine("Game Priority Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetDefaultCores(List<string> arguments)
        {
            if (arguments.Count >= minArgumentCount_SetCores)
            {
                List<int> cores = new List<int>();

                foreach (var str in arguments.Skip(1))
                {
                    int number;
                    if (int.TryParse(str, out number) && number <= highestCoreNumber && number >= 0)
                    {
                        cores.Add(number);
                    }
                    else
                    {
                        throw new ArgumentException();
                    }
                }
                cores.Sort();
                conf.DefaultConfig = new ProcessConfig(cores, conf.DefaultConfig.PriorityNumber);
                Console.WriteLine("Default Cores Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetDefaultPriority(List<string> arguments)
        {
            int priority;
            if (arguments.Count == exactArgumentCount_SetPriority && int.TryParse(arguments[1], out priority))
            {
                conf.DefaultConfig = new ProcessConfig(conf.DefaultConfig.CoreList, priority);
                Console.WriteLine("Default Priority Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetScanInterval(List<string> arguments)
        {
            int interval;
            if (arguments.Count == exactArgumentCount_SetScanInterval && int.TryParse(arguments[1], out interval))
            {
                conf.ScanInterval = interval;
                pow.SetTimer();
                Console.WriteLine("Scan Interval Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetAggressiveScanInterval(List<string> arguments)
        {
            int aggresiveScanInterval;
            if (arguments.Count == exactArgumentCount_SetAggressiveScanInterval && int.TryParse(arguments[1], out aggresiveScanInterval))
            {
                conf.AggressiveScanInterval = aggresiveScanInterval;
                Console.WriteLine("Aggressive Scan Interval Set!");
                Console.WriteLine($"Don't Forget to save!");
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetForceAggressiveScanInterval(List<string> arguments)
        {
            if (arguments.Count == exactArgumentCount_SetForceAggressiveScanInterval)
            {
                if (arguments[1] == "on" || arguments[1] == "yes" || arguments[1] == "true")
                {
                    conf.ForceAggressiveScan = true;
                    Console.WriteLine("Forced Aggressive Scans are now on!");
                    return;
                }
                else if (arguments[1] == "off" || arguments[1] == "no" || arguments[1] == "false")
                {
                    conf.ForceAggressiveScan = true;
                    Console.WriteLine("Forced Aggressive Scans are now off!");
                    return;
                }
            }

            throw new ArgumentException();
        }
        //////////////////////////////////////
        static void ShowDefaults()
        {
            Console.WriteLine("Defaults:");
            Console.WriteLine(conf.DefaultConfig);
        }
        //////////////////////////////////////
        static void ShowExceptions()
        {
            Console.WriteLine("Exceptions:");
            foreach (var item in conf.ProcessConfigs)
            {
                Console.WriteLine($"{item.Key} - {item.Value}");
            }
        }
        //////////////////////////////////////
        static void TogglePACT()
        {
            if (running)
            {
                ApplyDefaultConfig();
                Console.WriteLine("PACT Paused!");
            }
            else
            {
                ApplyCustomConfig();
                Console.WriteLine("PACT Resumed!");
            }
            running = !running;
        }

        static void ApplyCustomConfig()
        {
            pow.Config = conf;
            pow.Games = games;
            pow.SetTimer();
            pow.RunScan(true);
        }

        static void ApplyDefaultConfig()
        {
            pow.Config = pausedConf;
            pow.Games = new List<string>();
            pow.PauseTimer();
            pow.RunScan(true);
        }
        //////////////////////////////////////
        static void ShowHelp()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string textPath = path + "help.txt";
            if (File.Exists(textPath))
            {
                string text = File.ReadAllText(textPath);
                Console.WriteLine();
                Console.WriteLine(text);
            }
            else
            {
                Console.WriteLine("Cannot find help.txt, please check out https://github.com/sas41/ProcessAffinityControlTool");
            }
        }
        //////////////////////////////////////
        static void InvalidInputMessage()
        {
            Console.WriteLine();
            ConsoleColor bg = Console.BackgroundColor;
            ConsoleColor fg = Console.ForegroundColor;
            Console.BackgroundColor = ConsoleColor.DarkRed;
            Console.ForegroundColor = ConsoleColor.White;
            Console.WriteLine("Invalid Input!");
            Console.WriteLine("Type [help] or [?] for examples!");
            Console.BackgroundColor = bg;
            Console.ForegroundColor = fg;
        }
    }
}
