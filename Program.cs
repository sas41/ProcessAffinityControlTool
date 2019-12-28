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
        const string version = "1.0.1";

        const int minArgumentCount_AddException = 4;
        const int exactArgumentCount_RemoveException = 2;
        const int minArgumentCount_SetDefaultCores = 2;
        const int exactArgumentCount_SetDefaultPriority = 2;
        const int exactArgumentCount_SetScanInterval = 2;
        const int exactArgumentCount_SetAggressiveScanInterval = 2;
        const int exactArgumentCount_SetForceAggressiveScanInterval = 2;

        static ProcessOverwatch pow;
        static PACTConfig conf;
        static PACTConfig pausedConf;
        static bool running;
        static int highestCoreNumber;

        static void Main(string[] args)
        {
            pow = new ProcessOverwatch();
            conf = ReadConfig();
            pausedConf = new PACTConfig();
            running = true;
            highestCoreNumber = Environment.ProcessorCount;

            pow.Config = conf;
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
        static void SaveConfig(PACTConfig conf)
        {
            string json = JsonConvert.SerializeObject(conf, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "config.json";
            File.WriteAllText(configPath, json);
        }
        //////////////////////////////////////
        static void AddException(List<string> arguments)
        {
            int priority;
            List<int> cores = new List<int>();
            string exeName;

            if (arguments.Count >= minArgumentCount_AddException && int.TryParse(arguments[2], out priority))
            {
                foreach (var str in arguments.Skip(3))
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

                exeName = arguments[1];
                cores.Sort();

                if (exeName.Substring(exeName.Length - 4, 4) == ".exe")
                {
                    exeName = exeName.Substring(0, exeName.Length - 4);
                }

                if (conf.ProcessConfigs.ContainsKey(exeName))
                {
                    conf.ProcessConfigs[exeName] = new ProcessConfig(cores, priority);
                    Console.WriteLine($"Process [{exeName}] has been updated!");
                }
                else
                {
                    conf.ProcessConfigs.Add(exeName, new ProcessConfig(cores, priority));
                    Console.WriteLine($"Process [{exeName}] has been added!");
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
            string exeName;
            if (arguments.Count == exactArgumentCount_RemoveException && arguments[1].Replace(".exe","").Length > 0)
            {
                exeName = arguments[1];

                if (exeName.Substring(exeName.Length - 4, 4) == ".exe")
                {
                    exeName = exeName.Substring(0, exeName.Length - 4);
                }

                if (conf.ProcessConfigs.ContainsKey(exeName))
                {
                    conf.ProcessConfigs.Remove(exeName);
                    Console.WriteLine($"Process [{exeName}] has been removed!");
                }
                else
                {
                    Console.WriteLine($"Process [{exeName}] is not configured!");
                }
            }
            else
            {
                throw new ArgumentException();
            }
        }
        //////////////////////////////////////
        static void SetDefaultCores(List<string> arguments)
        {
            if (arguments.Count >= minArgumentCount_SetDefaultCores)
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
            if (arguments.Count == exactArgumentCount_SetDefaultPriority && int.TryParse(arguments[1], out priority))
            {
                conf.DefaultConfig = new ProcessConfig(conf.DefaultConfig.CoreList, priority);
                Console.WriteLine("Default Priority Set!");
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
            pow.SetTimer();
            pow.RunScan(true);
        }

        static void ApplyDefaultConfig()
        {
            pow.Config = pausedConf;
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
