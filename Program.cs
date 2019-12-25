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
        static ProcessOverwatch pow = new ProcessOverwatch();
        static PACTConfig conf;
        static PACTConfig pausedConf;

        static void Main(string[] args)
        {
            Console.WriteLine("P.A.C.T. v1.0, by Berk (SAS41) Alyamach.");
            Console.WriteLine("Type [help] or [?] for a valid list of commands.");

            pow = new ProcessOverwatch();
            conf = ReadConfig();
            pausedConf = new PACTConfig();
            pow.Config = conf;
            pow.SetTimer();


            bool running = true;

            while (true)
            {
                Console.WriteLine();
                Console.WriteLine("Enter your command:");
                string command = Console.ReadLine().ToLower();
                List<string> arguments = command.Split().ToList();
                try
                {
                    if (arguments[0] == "add")
                    {
                        string exeName = arguments[1].Replace(".exe", "");
                        int priority = int.Parse(arguments[2]);
                        List<int> cores = arguments.Skip(3).Select(x => int.Parse(x)).ToList();

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
                    else if (arguments[0] == "remove" || arguments[0] == "rem")
                    {
                        string exeName = arguments[1].Replace(".exe", "");
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
                    else if (arguments[0] == "default_cores" || arguments[0] == "dc")
                    {
                        List<int> cores = arguments.Skip(1).Select(x => int.Parse(x)).ToList();
                        conf.Default = new ProcessConfig(cores, conf.Default.PriorityNumber);
                    }
                    else if (arguments[0] == "default_priority" || arguments[0] == "dp")
                    {
                        conf.Default = new ProcessConfig(conf.Default.CoreList, int.Parse(arguments[1]));
                    }
                    else if (arguments[0] == "scan_interval" || arguments[0] == "si")
                    {
                        conf.ScanInterval = int.Parse(arguments[1]);
                        Console.WriteLine("Scan Interval Set!");
                    }
                    else if (arguments[0] == "aggressive_scan_interval" || arguments[0] == "asi")
                    {
                        conf.AggressiveScanInterval = int.Parse(arguments[1]);
                        Console.WriteLine("Aggressive Scan Interval Set!");
                    }
                    else if (arguments[0] == "force_aggressive_scan" || arguments[0] == "fas")
                    {
                        if (arguments[1] == "on" || arguments[1] == "yes" || arguments[1] == "true")
                        {
                            conf.ForceAggressiveScan = true;
                            Console.WriteLine("Forced Aggressive Scans are now on!");
                        }
                        else if (arguments[1] == "off" || arguments[1] == "no" || arguments[1] == "false")
                        {
                            conf.ForceAggressiveScan = true;
                            Console.WriteLine("Forced Aggressive Scans are now off!");
                        }
                        else
                        {
                            Console.WriteLine("Invalid Input!");
                        }
                    }
                    else if (arguments[0] == "show_defaults" || arguments[0] == "sd")
                    {
                        Console.WriteLine("Defaults:");
                        Console.WriteLine(conf.Default);
                    }
                    else if (arguments[0] == "show_exceptions" || arguments[0] == "se")
                    {
                        foreach (var item in conf.ProcessConfigs)
                        {
                            Console.WriteLine($"{item.Key} - {item.Value}");
                        }
                    }
                    else if (arguments[0] == "toggle")
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
                    else if (arguments[0] == "apply")
                    {
                        ApplyCustomConfig();
                        Console.WriteLine("Config Applied! Don't forget to save it!");
                    }
                    else if (arguments[0] == "save")
                    {
                        SaveConfig(conf);
                        Console.WriteLine("Config Saved! Don't forget to apply it!");
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
                }
                catch (FormatException fe)
                {
                    Console.WriteLine("Incorrect input! Please make sure your input is valid!");
                }
                catch (Exception ex)
                {
                    throw ex;
                }
            }
        }

        static void ApplyCustomConfig()
        {
            pow.Config = conf;
            pow.SetTimer();
            pow.RunScan();
        }

        static void ApplyDefaultConfig()
        {
            pow.Config = pausedConf;
            pow.PauseTimer();
            pow.RunScan();
        }

        static PACTConfig ReadConfig()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "/config.json";
            if (File.Exists(configPath))
            {
                string json = File.ReadAllText(configPath);
                return JsonConvert.DeserializeObject<PACTConfig>(json);
            }

            return new PACTConfig();
        }

        static void SaveConfig(PACTConfig conf)
        {
            string json = JsonConvert.SerializeObject(conf, Formatting.Indented);
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string configPath = path + "/config.json";
            File.WriteAllText(configPath, json);
        }

        static void ShowHelp()
        {
            string path = AppDomain.CurrentDomain.BaseDirectory;
            string textPath = path + "/help.txt";
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
    }
}
